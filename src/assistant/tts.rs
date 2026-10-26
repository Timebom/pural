use std::path::{Path, PathBuf};
use std::sync::Mutex;

use piper_rs::Piper;

use crate::assistant::interfaces::TTS;

/// Satisfies the TTS trait: synthesize(text) -> (float32 samples, rate).
pub struct PiperTTS {
    // Mutex because Piper::create takes &mut self
    voice: Mutex<Piper>,
}

impl PiperTTS {
    /// `model_path` should be the `.onnx` file.
    /// The matching `.onnx.json` config is expected next to it.
    pub fn new(model_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let onnx_path = model_path.as_ref().to_path_buf();

        if !onnx_path.exists() {
            return Err(format!("Piper voice not found: {}", onnx_path.display()).into());
        }

        // Config is usually model.onnx.json
        let config_path = PathBuf::from(format!("{}.json", onnx_path.display()));
        if !config_path.exists() {
            return Err(format!("Piper config not found: {}", config_path.display()).into());
        }

        let piper = Piper::new(&onnx_path, &config_path)
            .map_err(|e| format!("failed to load Piper model: {e}"))?;

        Ok(Self {
            voice: Mutex::new(piper),
        })
    }
}

impl TTS for PiperTTS {
    fn synthesize(&self, text: &str) -> (Vec<f32>, u32) {
        let mut voice = self.voice.lock().unwrap();

        // create(text, is_ssml, speaker_id, length_scale, noise_scale, noise_w)
        match voice.create(text, false, None, None, None, None) {
            Ok((samples, sample_rate)) => (samples, sample_rate),
            Err(_) => (Vec::new(), 22050), // safe fallback
        }
    }
}
