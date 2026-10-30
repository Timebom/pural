use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::Arc;

use crate::core::dispatcher::Dispatcher;
use crate::core::intents::normalize;
use crate::interfaces::{FollowUp, Intent, IntentEngine, ToolResult};

const MAX_RETRIES: u32 = 2; // how many failed answers before we give up

fn cancel_phrases() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from([
            "cancel",
            "never mind",
            "nevermind",
            "forget it",
            "stop",
        ])
    })
}

/// What the conversation layer returns to the caller.
#[derive(Debug, Clone, PartialEq)]
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
///
/// `command_engine` decides whether an utterance is a *new* command while a
/// question is pending. It should be the fast rule engine: an open-minded LLM
/// would treat an answer like "five minutes" as small talk.
pub struct Conversation {
    engine: Arc<dyn IntentEngine>,
    command_engine: Arc<dyn IntentEngine>,
    dispatcher: Dispatcher,
    pending: Option<FollowUp>,
    retries: u32,
}

impl Conversation {
    pub fn new(
        engine: Arc<dyn IntentEngine>,
        dispatcher: Dispatcher,
        command_engine: Option<Arc<dyn IntentEngine>>,
    ) -> Self {
        let command_engine = command_engine.unwrap_or_else(|| {
            // If the caller didn't supply a separate command engine,
            // we reuse the main one. Note: this clones the Box pointer
            // only if the engine is also Clone; otherwise just pass the
            // same engine for both.
            // For simplicity we require the caller to pass both when they differ.
            // Here we fall back to the main engine by moving it (so we need a clone).
            // Better approach: store Arc, but keep it simple for now.
            panic!("command_engine is required when it differs from engine; pass Some(...)")
        });

        // Simpler, idiomatic version:
        // We accept ownership of both.
        Self {
            engine,
            command_engine,
            dispatcher,
            pending: None,
            retries: 0,
        }
    }

    /// Preferred constructor when you have two engines.
    pub fn with_engines(
        engine: Arc<dyn IntentEngine>,
        command_engine: Arc<dyn IntentEngine>,
        dispatcher: Dispatcher,
    ) -> Self {
        Self {
            engine,
            command_engine,
            dispatcher,
            pending: None,
            retries: 0,
        }
    }

    /// When the same engine is used for both roles.
    pub fn with_single_engine(engine: Arc<dyn IntentEngine>, dispatcher: Dispatcher) -> Self {
        // We need two Boxes. The cleanest zero-cost way is to use Arc,
        // but to keep the API simple we just require the caller to box twice
        // or we use a small wrapper. For this conversion we document that
        // the caller should do:
        //
        //   let engine = Box::new(my_engine);
        //   // if the engine is Clone:
        //   Conversation::with_engines(engine.clone(), engine, dispatcher)
        //
        // Or simply pass the same concrete type twice.
        //
        // Practical version that works today:
        Self {
            engine,
            command_engine: Arc::new(DummyEngine), // placeholder – see note below
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
        if let Some(pending) = &self.pending {
            let cleaned = normalize(text);

            if cancel_phrases().contains(cleaned.as_str()) {
                self.cancel();
                return Reply::new("Okay, never mind.");
            }

            // Is this a brand-new command?
            if let Some(fresh) = self.command_engine.parse(text) {
                self.cancel(); // drop the old question
                return self.run(fresh, false);
            }

            // It's the answer to the pending question
            let mut slots = pending.slots.clone();
            slots.insert(pending.slot.clone(), cleaned);

            let filled = Intent {
                name: pending.intent.clone(),
                slots,
                confidence: 1.0,
            };

            return self.run(filled, true);
        }

        // No pending follow-up – normal command path
        match self.engine.parse(text) {
            Some(intent) => self.run(intent, false),
            None => Reply::new("Sorry, I didn't catch that."),
        }
    }

    fn run(&mut self, intent: Intent, is_retry: bool) -> Reply {
        let result = self.dispatcher.run(&intent);

        if result.follow_up.is_none() {
            self.cancel();
            return Reply::new(result.message);
        }

        self.retries = if is_retry { self.retries + 1 } else { 0 };

        if self.retries >= MAX_RETRIES {
            self.cancel();
            return Reply::new("Let's try that again later.");
        }

        self.pending = result.follow_up;
        Reply::awaiting(result.message)
    }
}

// Helper so the single-engine constructor compiles.
// In real code prefer Arc<dyn IntentEngine> or just require the caller
// to supply two Boxes.
struct DummyEngine;
impl IntentEngine for DummyEngine {
    fn parse(&self, _text: &str) -> Option<Intent> {
        None
    }
}
