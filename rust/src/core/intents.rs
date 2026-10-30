use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::interfaces::{Intent, IntentEngine}; // adjust path as needed

// Compiled regexes (lazy, thread-safe)
fn num_punct() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?<!\d)[.:]|[.:](?!\d)").unwrap())
}

fn hyphen_in_word() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?<=[a-z])-(?=[a-z])").unwrap())
}

fn unwanted() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^a-z0-9\s.:+\-*/%^']").unwrap())
}

fn wake_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(?:hey|ok|okay)\s+)?(?:assistant|pural)\b\s*").unwrap()
    })
}

fn polite() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:please|kindly)\b").unwrap())
}

fn spaces() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

/// Lowercase, strip punctuation and filler so rules see clean text.
pub fn normalize(text: &str) -> String {
    let mut text = text.to_lowercase().replace('’', "'");

    text = num_punct().replace_all(&text, " ").into_owned();
    text = hyphen_in_word().replace_all(&text, " ").into_owned();
    text = unwanted().replace_all(&text, " ").into_owned();
    text = spaces().replace_all(&text, " ").trim().to_string();

    text = wake_prefix().replace(&text, "").into_owned();
    text = polite().replace_all(&text, " ").into_owned();

    spaces().replace_all(&text, " ").trim().to_string()
}

// Rule + RuleEngine
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub pattern: Regex,
    pub confidence: f64,
}

pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a rule. Rules are tried in the order they are added.
    pub fn add(&mut self, name: impl Into<String>, pattern: &str, confidence: f64) {
        let re = Regex::new(pattern).expect("invalid rule pattern");
        self.rules.push(Rule {
            name: name.into(),
            pattern: re,
            confidence,
        });
    }

    /// Convenience: confidence defaults to 1.0
    pub fn add_default(&mut self, name: impl Into<String>, pattern: &str) {
        self.add(name, pattern, 1.0);
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentEngine for RuleEngine {
    fn parse(&self, text: &str) -> Option<Intent> {
        let cleaned = normalize(text);
        if cleaned.is_empty() {
            return None;
        }

        for rule in &self.rules {
            if let Some(caps) = rule.pattern.captures(&cleaned) {
                let mut slots = HashMap::new();

                // Named capture groups only
                for name in rule.pattern.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        slots.insert(name.to_string(), m.as_str().to_string());
                    }
                }

                return Some(
                    Intent::new(&rule.name)
                        .with_slots(slots)
                        .with_confidence(rule.confidence),
                );
            }
        }

        None
    }
}
