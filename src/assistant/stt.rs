use std::path::{Path, PathBuf};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::assistant::interfaces::STT;

/// Satisfies the STT trait: transcribe(audio) -> String.
/// Uses whisper.cpp via whisper-rs (ggml models, not faster-whisper CTranslate2).
pub struct FasterWhisperSTT {
    ctx: WhisperContext,
    // state is created per call; kept here only if you want to reuse later
    _model_name: String,
}

impl FasterWhisperSTT {
    /// `model` can be a short name like `"base.en"` or a full path to a `.bin` ggml file.
    /// If it's a short name, it looks under `download_root` for `ggml-{model}.bin`.
    pub fn new(
        model: &str,
        download_root: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model_path = resolve_model_path(model, download_root.as_ref())?;

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("invalid model path")?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("failed to load whisper model: {e}"))?;

        Ok(Self {
            ctx,
            _model_name: model.to_string(),
        })
    }
}

impl STT for FasterWhisperSTT {
    fn transcribe(&self, audio: &[f32]) -> String {
        // under ~0.1 seconds: nothing useful to transcribe
        if audio.len() < 1600 {
            return String::new();
        }

        let mut state = match self.ctx.create_state() {
            Ok(s) => s,
            Err(_) => return String::new(),
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        // beam_size=1 in Python ≈ greedy / best_of 1
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);

        if state.full(params, audio).is_err() {
            return String::new();
        }

        let mut parts = Vec::new();
        let n = state.full_n_segments();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                let text = seg.to_string();
                let t = text.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }

        parts.join(" ").trim().to_string()
    }
}

fn resolve_model_path(
    model: &str,
    download_root: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let p = Path::new(model);
    if p.exists() {
        return Ok(p.to_path_buf());
    }

    // common layouts:
    //   download_root/ggml-base.en.bin
    //   download_root/base.en.bin
    //   download_root/base.en/ggml-base.en.bin
    let candidates = [
        download_root.join(format!("ggml-{model}.bin")),
        download_root.join(format!("{model}.bin")),
        download_root.join(model).join(format!("ggml-{model}.bin")),
        download_root.join(model).join(format!("{model}.bin")),
    ];

    for c in candidates {
        if c.exists() {
            return Ok(c);
        }
    }

    Err(format!(
        "whisper model not found for '{model}' under {}",
        download_root.display()
    )
    .into())
}
