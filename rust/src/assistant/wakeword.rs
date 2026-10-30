// Cargo.toml
// oww-rs = "0.3"

use oww_rs::config::SpeechUnlockType;
use oww_rs::oww::{OwwModel, OWW_MODEL_CHUNK_SIZE};
use std::time::Duration;

/// These should match your `assistant::audio` constants.
/// `OWW_MODEL_CHUNK_SIZE` is 1280 (80 ms @ 16 kHz).
pub const SAMPLE_RATE: u32 = 16_000;
pub const FRAME_SAMPLES: usize = OWW_MODEL_CHUNK_SIZE as usize; // 1280

/// float32 in [-1.0, 1.0] → i16, clipped (kept for compatibility;
/// oww-rs works directly on f32).
pub fn to_int16(frame: &[f32]) -> Vec<i16> {
    frame
        .iter()
        .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect()
}

pub struct WakeWordDetector {
    model: OwwModel,
    threshold: f32,
    cooldown: Duration,
    last_fire: Option<std::time::Instant>,
}

impl WakeWordDetector {
    /// `model` – one of the built-in variants, e.g. `SpeechUnlockType::OpenWakeWordAlexa`
    /// or a custom model loaded with `OwwModel::from_file`.
    pub fn new(
        model_type: SpeechUnlockType,
        threshold: f32,
        cooldown_seconds: f32,
    ) -> Result<Self, String> {
        let model = OwwModel::new(model_type, threshold)?;

        Ok(Self {
            model,
            threshold,
            cooldown: Duration::from_secs_f32(cooldown_seconds),
            last_fire: None,
        })
    }

    /// Convenience constructor with the same defaults as the Python version.
    /// Uses the built-in Alexa model; change the variant if you want another.
    pub fn with_defaults() -> Result<Self, String> {
        Self::new(SpeechUnlockType::OpenWakeWordAlexa, 0.5, 2.0)
    }

    /// Load a custom `.onnx` wake-word model from disk.
    pub fn from_file(
        path: impl AsRef<std::path::Path>,
        unlock_word: impl Into<String>,
        threshold: f32,
        cooldown_seconds: f32,
    ) -> std::io::Result<Self> {
        let model = OwwModel::from_file(path, unlock_word.into(), threshold)?;

        Ok(Self {
            model,
            threshold,
            cooldown: Duration::from_secs_f32(cooldown_seconds),
            last_fire: None,
        })
    }

    /// Wake-word probability for one 80 ms frame (always updates the model).
    pub fn score(&mut self, frame: &[f32]) -> f32 {
        // oww-rs expects owned Vec<f32> of length OWW_MODEL_CHUNK_SIZE
        let chunk = if frame.len() == FRAME_SAMPLES {
            frame.to_vec()
        } else {
            // pad / truncate if the caller gives a slightly different size
            let mut v = frame.to_vec();
            v.resize(FRAME_SAMPLES, 0.0);
            v
        };

        let detection = self.model.detection(chunk);
        detection.probability
    }

    /// `true` once per utterance of the wake phrase (respects cooldown).
    pub fn detect(&mut self, frame: &[f32]) -> bool {
        let detection = {
            let chunk = if frame.len() == FRAME_SAMPLES {
                frame.to_vec()
            } else {
                let mut v = frame.to_vec();
                v.resize(FRAME_SAMPLES, 0.0);
                v
            };
            self.model.detection(chunk)
        };

        if !detection.detected {
            return false;
        }

        // Simple cooldown so we don't re-trigger immediately
        let now = std::time::Instant::now();
        if let Some(last) = self.last_fire {
            if now.duration_since(last) < self.cooldown {
                return false;
            }
        }

        self.last_fire = Some(now);
        true
    }

    pub fn reset(&mut self) {
        self.last_fire = None;
        // OwwModel keeps its own internal detection buffer;
        // recreating it is the safest “full reset” if you need one.
    }
}

/// Consume frames until the wake phrase is heard.
/// Returns `false` if the stream ends without a detection.
pub fn wait_for_wakeword<I>(detector: &mut WakeWordDetector, source: I) -> bool
where
    I: IntoIterator<Item = Vec<f32>>,
{
    for frame in source {
        if detector.detect(&frame) {
            return true;
        }
    }
    false
}
