use regex::Regex;
use std::collections::HashMap;
use once_cell::sync::Lazy;

use crate::assistant::interfaces::{Intent, IntentEngine}; // adjust path if needed

// Precompiled regexes
static NUM_PUNCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:^|[^\d])[.:]|[.:](?:[^\d]|$)").unwrap()
});

static HYPHEN_IN_WORD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([a-z])-([a-z])").unwrap()
});

static UNWANTED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[^a-z0-9\s.:+\-*/%^']").unwrap()
});

static WAKE_PREFIX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:(?:hey|ok|okay)\s+)?(?:assistant|pural)\b\s*").unwrap()
});

static POLITE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:please|kindly)\b").unwrap()
});

static SPACES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\s+").unwrap()
});

/// Lowercase, strip punctuation and filler so rules see clean text.
pub fn normalize(text: &str) -> String {
    let mut text = text.to_lowercase().replace('’', "'");

    text = NUM_PUNCT.replace_all(&text, " ").into_owned();
    text = HYPHEN_IN_WORD.replace_all(&text, " ").into_owned();
    text = UNWANTED.replace_all(&text, " ").into_owned();
    text = SPACES.replace_all(&text, " ").trim().to_string();

    text = WAKE_PREFIX.replace(&text, "").into_owned();
    text = POLITE.replace_all(&text, " ").into_owned();

    SPACES.replace_all(&text, " ").trim().to_string()
}

// Rule
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub pattern: Regex,
    pub confidence: f32,
}

impl Rule {
    pub fn new(name: impl Into<String>, pattern: &str, confidence: f32) -> Result<Self, regex::Error> {
        Ok(Self {
            name: name.into(),
            pattern: Regex::new(pattern)?,
            confidence,
        })
    }
}

// RuleEngine
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a rule. Rules are tried in the order they are added.
    pub fn add(&mut self, name: impl Into<String>, pattern: &str, confidence: f32) -> Result<(), regex::Error> {
        self.rules.push(Rule::new(name, pattern, confidence)?);
        Ok(())
    }

    /// Satisfies the IntentEngine trait: parse(text) -> Option<Intent>
    pub fn parse(&self, text: &str) -> Option<Intent> {
        let cleaned = normalize(text);
        if cleaned.is_empty() {
            return None;
        }

        for rule in &self.rules {
            if let Some(caps) = rule.pattern.captures(&cleaned) {
                let mut slots = HashMap::new();

                for name in rule.pattern.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        slots.insert(name.to_string(), m.as_str().to_string());
                    }
                }

                return Some(Intent {
                    name: rule.name.clone(),
                    slots,
                    confidence: rule.confidence,
                });
            }
        }

        None
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentEngine for RuleEngine {
    fn parse(&self, text: &str) -> Option<Intent> {
        self.parse(text)
    }
}
