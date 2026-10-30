use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::LazyLock;

use crate::core::intents::normalize;
use crate::core::parsing::NUMBER_WORDS;
use crate::interfaces::Intent;
use crate::llm::base::{LLMBackend, LLMError, Message, Role};

// intent -> name of its single slot (None = takes no slot).
// One source of truth: the schema's enum and the arguments -> slot mapping
// both come from this map.
static INTENT_SLOTS: LazyLock<HashMap<&'static str, Option<&'static str>>> = LazyLock::new(|| {
    HashMap::from([
        ("set_timer", Some("duration")),
        ("get_time", None),
        ("get_date", None),
        ("set_volume", Some("level")),
        ("volume_up", None),
        ("volume_down", None),
        ("calculate", Some("expression")),
        ("tell_joke", None),
        ("get_weather", Some("location")),
        ("web_search", Some("query")),
    ])
});

const CHAT: &str = "chat";

static ROUTE_SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    let mut intents: Vec<Value> = INTENT_SLOTS.keys().map(|k| Value::String(k.to_string())).collect();
    intents.push(Value::String(CHAT.to_string()));

    json!({
        "type": "object",
        "properties": {
            "intent": {
                "type": "string",
                "enum": intents
            },
            "argument": { "type": "string" }
        },
        "required": ["intent", "argument"]
    })
});

static ROUTER: LazyLock<Message> = LazyLock::new(|| {
    Message {
        role: Role::System,
        content: r#"You route requests for a voice assistant. Reply with JSON: an intent and an argument.
Intents:
- set_timer: start a countdown, or wake/remind me after some time. argument = the duration, e.g. "5 minutes".
- get_time: the user asks what time it is.
- get_date: the user asks for today's date, the day of the week, or the year.
- set_volume: set the volume to a specific level. argument = the level, e.g. "40".
- volume_up: the user wants more volume: louder, turn it up, too quiet, too soft, can't hear.
- volume_down: the user wants less volume: quieter, turn it down, too loud.
- calculate: do arithmetic. argument = the expression, e.g. "12 times 3".
- tell_joke: ONLY when the user asks for a joke or something to make them laugh.
- get_weather: the user asks about the weather or a forecast. argument = the place and/or "tomorrow" if mentioned, else empty.
- web_search: ONLY when the user explicitly asks to search, google or look something up online. argument = what to search for.
- chat: everything else, including questions, facts, trivia, stories, advice, greetings and small talk.
Rules:
- A question about facts, people, places, science, trivia or advice is ALWAYS chat, even if it starts with "what's the".
- The argument is a short phrase copied from the request, or empty. For chat it is ALWAYS empty.
- For set_timer the argument must include the unit: seconds, minutes or hours.
Examples:
"what's the weather in Paris" -> {"intent": "get_weather", "argument": "Paris"}
"is it going to rain tomorrow" -> {"intent": "get_weather", "argument": "tomorrow"}
"search the web for the best pizza recipe" -> {"intent": "web_search", "argument": "the best pizza recipe"}
"what's the capital of Italy" -> {"intent": "chat", "argument": ""}
"what's the population of India" -> {"intent": "chat", "argument": ""}
"what's the tallest mountain" -> {"intent": "chat", "argument": ""}
"give me an interesting fact about whales" -> {"intent": "chat", "argument": ""}
"tell me a story about a pirate" -> {"intent": "chat", "argument": ""}
"what year is it" -> {"intent": "get_date", "argument": ""}
"I can't hear you" -> {"intent": "volume_up", "argument": ""}
"it's hurting my ears" -> {"intent": "volume_down", "argument": ""}
"remind me in half an hour" -> {"intent": "set_timer", "argument": "half an hour"}
"set a timer for forty five seconds" -> {"intent": "set_timer", "argument": "forty five seconds"}
"what's two hundred divided by four" -> {"intent": "calculate", "argument": "two hundred divided by four"}
"what's eleven plus nine" -> {"intent": "calculate", "argument": "eleven plus nine"}
"set the volume to forty" -> {"intent": "set_volume", "argument": "forty"}
"put the volume at forty five" -> {"intent": "set_volume", "argument": "forty five"}
"#
        .to_string(),
    }
});

const CHAT_RULES: &str = "You are a friendly voice assistant. Answer in one or two short sentences, \
in plain spoken language with no lists, markdown or emoji. You cannot check \
live information such as weather or news; say so briefly if asked.";

fn chat_system() -> Message {
    let now = chrono::Local::now();
    Message {
        role: Role::System,
        content: format!(
            "{CHAT_RULES} Today's date is {}. You can tell stories, jokes and fun facts.",
            now.format("%A, %B %d, %Y")
        ),
    }
}

// Words that must appear in the request before a tool intent is believed.
static EVIDENCE: LazyLock<HashMap<&'static str, Regex>> = LazyLock::new(|| {
    let numbers = {
        let mut words: Vec<&str> = NUMBER_WORDS.iter().copied().collect();
        words.sort_unstable();
        words.join("|")
    };

    let volume_topic = r"volume|sound|loud\w*|quiet\w*|soft\w*|hear\w*|noise|noisy|music|ears?|mute|raise|lower|boost";

    let patterns: [(&str, String); 10] = [
        ("set_timer", r"timer|countdown|alarm|remind\w*|wake|seconds?|minutes?|hours?".into()),
        ("get_time", r"the\s+time|time\s+(?:is\s+it|it\s+is|now)|clock|current\s+time".into()),
        ("get_date", r"date|day|today|month|year|week".into()),
        ("set_volume", format!(r"{volume_topic}|level")),
        ("volume_up", volume_topic.into()),
        ("volume_down", volume_topic.into()),
        (
            "calculate",
            format!(r"\d+|{numbers}|plus|minus|times|divid\w*|multipl\w*|percent|squared|over"),
        ),
        ("tell_joke", r"jok\w*|laugh\w*|funny|puns?|humou?r".into()),
        (
            "get_weather",
            r"weather|forecast|rain\w*|snow\w*|temperature|sunny|cloudy|windy|umbrella|humid\w*|hot|cold".into(),
        ),
        ("web_search", r"search\w*|google|look|find|web|internet|online".into()),
    ];

    patterns
        .into_iter()
        .map(|(k, v)| {
            let re = Regex::new(&format!(r"\b(?:{v})\b")).expect("invalid evidence regex");
            (k, re)
        })
        .collect()
});

static INTENT_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""intent"\s*:\s*"(\w+)""#).unwrap());

/// Validate the router's JSON. Returns `(intent, argument)`, or `None` if unusable.
pub fn parse_route(raw: &str) -> Option<(String, String)> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => {
            let intent = map.get("intent")?.as_str()?;
            if intent != CHAT && !INTENT_SLOTS.contains_key(intent) {
                return None;
            }
            let argument = map
                .get("argument")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            Some((intent.to_string(), argument))
        }
        _ => {
            // Truncated output (a runaway argument hit max_tokens).
            // `intent` comes first, so it may still be readable.
            // Trust it only when no argument is needed.
            if let Some(caps) = INTENT_FIELD.captures(raw) {
                let name = caps.get(1)?.as_str();
                if name == CHAT
                    || INTENT_SLOTS
                        .get(name)
                        .map(|slot| slot.is_none())
                        .unwrap_or(false)
                {
                    return Some((name.to_string(), String::new()));
                }
            }
            None
        }
    }
}

fn to_intent(intent: &str, argument: &str) -> Intent {
    let mut slots = HashMap::new();
    if let Some(Some(slot)) = INTENT_SLOTS.get(intent) {
        if !argument.is_empty() {
            slots.insert(slot.to_string(), argument.to_string());
        }
    }
    // rules say 1.0; the LLM is less sure
    Intent {
        name: intent.to_string(),
        slots,
        confidence: 0.7,
    }
}

/// Satisfies IntentEngine. Routes with one constrained call; chats with a second.
pub struct LLMEngine {
    backend: Box<dyn LLMBackend>,
    history: VecDeque<Message>,
    history_capacity: usize,
}

impl LLMEngine {
    pub fn new(backend: Box<dyn LLMBackend>, history_turns: usize) -> Self {
        Self {
            backend,
            history: VecDeque::with_capacity(2 * history_turns),
            history_capacity: 2 * history_turns,
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
    }

    pub fn parse(&mut self, text: &str) -> Option<Intent> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        let raw = match self.backend.chat(
            &[ROUTER.clone(), Message::user(text)],
            None,
            Some(&ROUTE_SCHEMA),
        ) {
            Ok(resp) => resp.text,
            Err(_) => return None,
        };

        let (intent, argument) = match parse_route(&raw) {
            Some(route) => route,
            None => return None,
        };

        if intent == CHAT {
            return self.chat(text);
        }

        let evidence = EVIDENCE.get(intent.as_str())?;
        if !evidence.is_match(&normalize(text)) {
            return self.chat(text);
        }

        Some(to_intent(&intent, &argument))
    }

    fn chat(&mut self, text: &str) -> Option<Intent> {
        let mut messages = Vec::with_capacity(2 + self.history.len());
        messages.push(chat_system());
        messages.extend(self.history.iter().cloned());
        messages.push(Message::user(text));

        let answer = match self.backend.chat(&messages, None, None) {
            Ok(resp) => resp.text,
            Err(_) => return None,
        };

        if answer.is_empty() {
            return None;
        }

        // Keep history within capacity (equivalent to deque maxlen)
        if self.history.len() >= self.history_capacity {
            self.history.pop_front();
            if self.history.len() >= self.history_capacity {
                self.history.pop_front();
            }
        }
        self.history.push_back(Message::user(text));
        self.history.push_back(Message::assistant(&answer));

        let mut slots = HashMap::new();
        slots.insert("text".to_string(), answer);

        Some(Intent {
            name: "chat_reply".to_string(),
            slots,
            confidence: 0.7,
        })
    }
}
