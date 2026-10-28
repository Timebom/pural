import json
import logging
import re
from collections import deque
from datetime import datetime

from assistant.core.intents import normalize
from assistant.core.parsing import NUMBER_WORDS
from assistant.interfaces import Intent
from assistant.llm.base import LLMBackend, LLMError, Message

log = logging.getLogger(__name__)

# intent -> name of its single slot (None = takes no slot). One source of truth:
# the schema's enum and the arguments -> slot mapping both come from this dict.
INTENT_SLOTS: dict[str, str | None] = {
    "set_timer": "duration",
    "get_time": None,
    "get_date": None,
    "set_volume": "level",
    "volume_up": None,
    "volume_down": None,
    "calculate": "expression",
    "tell_joke": None,
}
CHAT = "chat"

ROUTE_SCHEMA = {
    "type": "object",
    "properties": {
        "intent": {"type": "string", "enum": [*INTENT_SLOTS, CHAT]},
        "argument": {"type": "string"},
    },
    "required": ["intent", "argument"],
}

ROUTER = Message("system", """You route requests for a voice assistant. Reply with JSON: an intent and an argument.

Intents:
- set_timer: start a countdown, or wake/remind me after some time. argument = the duration, e.g. "5 minutes".
- get_time: the user asks what time it is.
- get_date: the user asks for today's date, the day of the week, or the year.
- set_volume: set the volume to a specific level. argument = the level, e.g. "40".
- volume_up: the user wants more volume: louder, turn it up, too quiet, too soft, can't hear.
- volume_down: the user wants less volume: quieter, turn it down, too loud.
- calculate: do arithmetic. argument = the expression, e.g. "12 times 3".
- tell_joke: ONLY when the user asks for a joke or something to make them laugh.
- chat: everything else, including questions, facts, trivia, stories, advice, greetings and small talk.

Rules:
- A question about facts, people, places, science, trivia or advice is ALWAYS chat, even if it starts with "what's the".
- The argument is a short phrase copied from the request, or empty. For chat it is ALWAYS empty.

Examples:
"what's the population of India" -> {"intent": "chat", "argument": ""}
"what's the tallest mountain" -> {"intent": "chat", "argument": ""}
"give me an interesting fact about whales" -> {"intent": "chat", "argument": ""}
"tell me a story about a pirate" -> {"intent": "chat", "argument": ""}
"what year is it" -> {"intent": "get_date", "argument": ""}
"I can't hear you" -> {"intent": "volume_up", "argument": ""}
"it's hurting my ears" -> {"intent": "volume_down", "argument": ""}
"remind me in half an hour" -> {"intent": "set_timer", "argument": "half an hour"}
"what's two hundred divided by four" -> {"intent": "calculate", "argument": "two hundred divided by four"}
"set the volume to forty" -> {"intent": "set_volume", "argument": "forty"}"""
)

_CHAT_RULES = (
    "You are a friendly voice assistant. Answer in one or two short sentences, "
    "in plain spoken language with no lists, markdown or emoji. You cannot check "
    "live information such as weather or news; say so briefly if asked."
)


def _chat_system() -> Message:
    now = datetime.now()
    return Message("system", f"{_CHAT_RULES} Current date and time: {now:%A, %B %d, %Y, %H:%M}.")


# Words that must appear in the request before a tool intent is believed.
_NUMBERS = "|".join(sorted(NUMBER_WORDS))
_VOLUME_TOPIC = (r"volume|sound|loud\w*|quiet\w*|soft\w*|hear\w*|noise|noisy|music|ears?"
                 r"|mute|raise|lower|boost")
_EVIDENCE = {
    "set_timer": r"timer|countdown|alarm|remind\w*|wake|seconds?|minutes?|hours?",
    "get_time": r"time|clock",
    "get_date": r"date|day|today|month|year|week",
    "set_volume": _VOLUME_TOPIC + r"|level",
    "volume_up": _VOLUME_TOPIC,
    "volume_down": _VOLUME_TOPIC,
    "calculate": rf"\d+|{_NUMBERS}|plus|minus|times|divid\w*|multipl\w*|percent|squared|over",
    "tell_joke": r"jok\w*|laugh\w*|funny|puns?|humou?r",
}
_EVIDENCE = {k: re.compile(rf"\b(?:{v})\b") for k, v in _EVIDENCE.items()}

_INTENT_FIELD = re.compile(r'"intent"\s*:\s*"(\w+)"')


def parse_route(raw: str) -> tuple[str, str] | None:
    """Validate the router's JSON. Returns (intent, argument), or None if unusable."""
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        # Truncated output (a runaway argument hit max_tokens). `intent` comes first,
        # so it may still be readable. Trust it only when no argument is needed.
        match = _INTENT_FIELD.search(raw)
        if match:
            name = match.group(1)
            if name == CHAT or (name in INTENT_SLOTS and INTENT_SLOTS[name] is None):
                return name, ""
        return None
    if not isinstance(data, dict):
        return None
    intent = data.get("intent")
    if intent != CHAT and intent not in INTENT_SLOTS:
        return None
    argument = data.get("argument", "")
    return intent, argument.strip() if isinstance(argument, str) else ""


def _to_intent(intent: str, argument: str) -> Intent:
    slot = INTENT_SLOTS[intent]
    slots = {slot: argument} if slot and argument else {} # junk args on no-slot intents are dropped
    return Intent(intent, slots, confidence=0.7) # rules say 1.0; the LLM is less sure


class LLMEngine:
    """Statisfies IntentEngine. Routes with one constrained call; chats with a seconds,"""

    def __init__(self, backend: LLMBackend, history_turns: int = 4) -> None:
        self._backend = backend
        self._history: deque[Message] = deque(maxlen=2 * history_turns)

    def reset(self) -> None:
        self._history.clear()

    def parse(self, text: str) -> Intent | None:
        text = text.strip()
        if not text:
            return None
        try:
            raw = self._backend.chat([ROUTER, Message("user", text)],
               json_schema=ROUTE_SCHEMA).text
            route = parse_route(raw)
            if route is None:
                log.warning("unusable router output: %r", raw[:100])
                return None
            intent, argument = route
            if intent == CHAT:
                return self._chat(text)
            if not _EVIDENCE[intent].search(normalize(text)):
                log.info("rejected %s for %r: no supporting words, treating as chat",
                    intent, text)
                return self._chat(text)
            return _to_intent(intent, argument)
        except LLMError as e:
            log.warning("LLM unavailable: %s", e)
            return None

    def _chat(self, text: str) -> Intent | None:
        messages = [_chat_system(), *self._history, Message("user", text)]
        answer = self._backend.chat(messages).text
        if not answer:
            return None
        self._history.append(Message("user", text))
        self._history.append(Message("assistant", answer))
        return Intent("chat_reply", {"text": answer}, confidence=0.7)
