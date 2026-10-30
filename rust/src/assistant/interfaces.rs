use std::collections::HashMap;

/// What the user wants, independent of how we figured it out.
#[derive(Debug, Clone)]
pub struct Intent {
    pub name: String,
    pub slots: HashMap<String, String>,
    pub confidence: f32,
}

impl Intent {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            slots: HashMap::new(),
            confidence: 1.0,
        }
    }

    pub fn with_slots(mut self, slots: HashMap<String, String>) -> Self {
        self.slots = slots;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }
}

/// "I need one more slot": the next utterance is the answer to this.
#[derive(Debug, Clone, PartialEq)]
pub struct FollowUp {
    /// Intent to re-run, e.g. "set_timer"
    pub intent: String,
    /// Slot the answer fills, e.g. "duration"
    pub slot: String,
    /// Slots already known
    pub slots: HashMap<String, String>,
}

impl FollowUp {
    pub fn new(intent: impl Into<String>, slot: impl Into<String>) -> Self {
        Self {
            intent: intent.into(),
            slot: slot.into(),
            slots: HashMap::new(),
        }
    }

    pub fn with_slots(mut self, slots: HashMap<String, String>) -> Self {
        self.slots = slots;
        self
    }
}

/// What a tool returns: structured for code, readable for TTS.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub ok: bool,
    pub message: String,
    pub data: HashMap<String, String>, // or serde_json::Value if you need richer data
    pub follow_up: Option<FollowUp>,
}

impl ToolResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            data: HashMap::new(),
            follow_up: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            data: HashMap::new(),
            follow_up: None
        }
    }

    pub fn with_data(mut self, data: HashMap<String, String>) -> Self {
        self.data = data;
        self
    }

    pub fn with_follow_up(mut self, follow_up: FollowUp) -> Self {
        self.follow_up = Some(follow_up);
        self
    }
}

/// Speech-to-Text interface.
/// `audio` is expected to be: float32, mono, 16 kHz, values in [-1.0, 1.0]
pub trait STT {
    fn transcribe(&self, audio: &[f32]) -> String;
}

/// Text-to-Speech interface.
/// Returns (float32 mono samples, sample_rate)
pub trait TTS {
    fn synthesize(&self, text: &str) -> (Vec<f32>, u32);
}

/// Intent parsing interface.
pub trait IntentEngine {
    fn parse(&self, text: &str) -> Option<Intent>;
}
