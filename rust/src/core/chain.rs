use crate::interfaces::{Intent, IntentEngine};

/// Tries each engine in order; the first one that understands wins.
pub struct ChainEngine {
    engines: Vec<Box<dyn IntentEngine>>,
}

impl ChainEngine {
    pub fn new(engines: Vec<Box<dyn IntentEngine>>) -> Self {
        Self { engines }
    }

    /// Convenience constructor from a list of engines.
    pub fn from_engines<I>(engines: I) -> Self
    where
        I: IntoIterator<Item = Box<dyn IntentEngine>>,
    {
        Self {
            engines: engines.into_iter().collect(),
        }
    }
}

impl IntentEngine for ChainEngine {
    fn parse(&self, text: &str) -> Option<Intent> {
        for engine in &self.engines {
            if let Some(intent) = engine.parse(text) {
                return Some(intent);
            }
        }
        None
    }
}
