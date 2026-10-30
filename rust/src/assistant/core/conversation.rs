// src/core/conversation.rs

use crate::assistant::core::dispatcher::Dispatcher;
use crate::assistant::core::intents::normalize;
use crate::assistant::interfaces::{FollowUp, Intent, IntentEngine, ToolResult};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

fn cancel_phrases() -> &'static HashSet<&'static str> {
    static PHRASES: OnceLock<HashSet<&'static str>> = OnceLock::new();
    PHRASES.get_or_init(|| {
        HashSet::from([
            "cancel",
            "never mind",
            "nevermind",
            "forget it",
            "stop",
        ])
    })
}

const MAX_RETRIES: u32 = 2; // how many failed answers before we give up

#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    /// `true` → caller should listen again (no wake word required)
    pub awaiting_answer: bool,
}

impl Reply {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            awaiting_answer: false,
        }
    }

    pub fn awaiting(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            awaiting_answer: true,
        }
    }
}

/// Remembers whether the assistant is waiting for an answer.
pub struct Conversation<E, D>
where
    E: IntentEngine,
    D: crate::interfaces::Dispatcher,
{
    engine: E,
    dispatcher: D,
    pending: Option<FollowUp>,
    retries: u32,
}

impl<E, D> Conversation<E, D>
where
    E: IntentEngine,
    D: crate::interfaces::Dispatcher,
{
    pub fn new(engine: E, dispatcher: D) -> Self {
        Self {
            engine,
            dispatcher,
            pending: None,
            retries: 0,
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
        self.retries = 0;
    }

    pub fn respond(&mut self, text: &str) -> Reply {
        // We are currently waiting for the answer to a previous question
        if let Some(pending) = self.pending.clone() {
            let cleaned = normalize(text);

            if cancel_phrases().contains(cleaned.as_str()) {
                self.cancel();
                return Reply::new("Okay, never mind.");
            }

            // Try to parse a completely new command
            if let Some(fresh) = self.engine.parse(text) {
                // New command → abandon the old pending question
                self.cancel();
                return self.run(fresh, false);
            }

            // Not a command → treat the utterance as the missing slot value
            let mut slots = pending.slots;
            slots.insert(pending.slot.clone(), cleaned);

            let filled = Intent {
                name: pending.intent,
                slots,
                confidence: 1.0,
            };
            return self.run(filled, true);
        }

        // No pending follow-up – normal command path
        match self.engine.parse(text) {
            None => Reply::new("Sorry, I didn't catch that."),
            Some(intent) => self.run(intent, false),
        }
    }

    fn run(&mut self, intent: Intent, is_retry: bool) -> Reply {
        let result = self.dispatcher.run(&intent);

        // Tool finished without asking anything else
        if result.follow_up.is_none() {
            self.cancel();
            return Reply::new(result.message);
        }

        // Tool asked a follow-up question
        self.retries = if is_retry {
            self.retries.saturating_add(1)
        } else {
            0
        };

        if self.retries >= MAX_RETRIES {
            self.cancel();
            return Reply::new("Let's try that again later.");
        }

        self.pending = result.follow_up;
        Reply::awaiting(result.message)
    }
}
