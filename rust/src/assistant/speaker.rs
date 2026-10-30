use std::sync::Mutex;
use crate::assistant::audio::play;
use crate::assistant::interfaces::TTS;

/// Speak text one utterance at a time, safe to call from any thread.
pub struct Speaker {
    tts: Box<dyn TTS + Send + Sync>,
    lock: Mutex<()>,
}

impl Speaker {
    pub fn new(tts: impl TTS + Send + Sync + 'static) -> Self {
        Self {
            tts: Box::new(tts),
            lock: Mutex::new(()),
        }
    }

    pub fn say(&self, text: &str) {
        // A timer can't talk over a reply
        let _guard = self.lock.lock().unwrap();

        let (samples, rate) = self.tts.synthesize(text);
        let _ = play(&samples, rate);
    }
}
