use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

use ndarray::{Array1, Array2, Array3};
use ort::{session::Session, value::Tensor};

use crate::assistant::audio::{FRAME_SAMPLES, SAMPLE_RATE};

const WINDOW: usize = 512; // samples per model call at 16 kHz (32 ms)
const CONTEXT: usize = 64; // samples carried over from the previous window
const FRAME_SECONDS: f32 = FRAME_SAMPLES as f32 / SAMPLE_RATE as f32; // 0.08 s

pub struct SileroVAD {
    session: Mutex<Session>,
    threshold: f32,
    state: Array3<f32>,     // (2, 1, 128)
    context: Array2<f32>,   // (1, 64)
    pending: Vec<f32>,
    last_prob: f32,
}

impl SileroVAD {
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Result<Self, Box<dyn std::error::Error>> {
        let path = model_path.as_ref();
        if !path.exists() {
            return Err(format!("VAD model not found: {}", path.display()).into());
        }

        let session = Session::builder()?
            .with_intra_threads(1)?
            .commit_from_file(path)?;

        let mut vad = Self {
            session: Mutex::new(session),
            threshold: threshold,
            state: Array3::zeros((2, 1, 128)),
            context: Array2::zeros((1, CONTEXT)),
            pending: Vec::new(),
            last_prob: 0.0,
        };
        vad.reset();
        Ok(vad)
    }

    /// Forget all audio history. Call between separate utterances.
    pub fn reset(&mut self) {
        self.state = Array3::zeros((2, 1, 128));
        self.context = Array2::zeros((1, CONTEXT));
        self.pending.clear();
        self.last_prob = 0.0;
    }

    fn run_window(&mut self, window: &[f32]) -> Result<f32, Box<dyn std::error::Error>> {
        // x = concat(context, window) → shape (1, 576)
        let mut x = Array2::<f32>::zeros((1, CONTEXT + WINDOW));
        x.slice_mut(ndarray::s![0, 0..CONTEXT])
            .assign(&self.context.slice(ndarray::s![0, ..]));
        for (i, &s) in window.iter().enumerate() {
            x[[0, CONTEXT + i]] = s;
        }

        let mut session = self.session.lock().unwrap();

        let input = Tensor::from_array(x.clone())?;
        let state = Tensor::from_array(self.state.clone())?;
        let sr = Tensor::from_array(Array1::from_vec(vec![SAMPLE_RATE as i64]))?;

        let outputs = session.run(ort::inputs! {
            "input" => input,
            "state" => state,
            "sr" => sr,
        })?;

        // outputs[0] = probability, outputs[1] = new state
        let prob_tensor = outputs[0].try_extract_tensor::<f32>()?;
        let state_tensor = outputs[1].try_extract_tensor::<f32>()?;

        let prob = prob_tensor.1[0]; // first value
        self.state = Array3::from_shape_vec((2, 1, 128), state_tensor.1.to_vec())?;

        // keep last CONTEXT samples as next context
        self.context = x.slice(ndarray::s![.., (x.ncols() - CONTEXT)..]).to_owned();

        Ok(prob)
    }

    /// Feed any amount of float32 mono audio; returns the highest speech probability
    /// among the complete 512-sample windows it contained.
    pub fn get_speech_probability(&mut self, chunk: &[f32]) -> f32 {
        self.pending.extend_from_slice(chunk);

        let mut probs = Vec::new();
        while self.pending.len() >= WINDOW {
            let window: Vec<f32> = self.pending.drain(..WINDOW).collect();
            if let Ok(p) = self.run_window(&window) {
                probs.push(p);
            }
        }

        if let Some(&max_p) = probs.iter().max_by(|a, b| a.partial_cmp(b).unwrap()) {
            self.last_prob = max_p;
        }

        self.last_prob
    }

    pub fn is_speech(&mut self, chunk: &[f32]) -> bool {
        self.get_speech_probability(chunk) >= self.threshold
    }
}

/// Capture one utterance. Return float32 mono audio, or `None` if nobody spoke.
pub fn record_until_silence(
    vad: &mut SileroVAD,
    source: impl Iterator<Item = Vec<f32>>,
    wait_seconds: f32,     // give up if nobody speaks
    silence_seconds: f32,  // quiet this long = utterance finished
    max_seconds: f32,      // hard cap on one command
    preroll_frames: usize, // frames kept before speech starts
) -> Option<Vec<f32>> {
    vad.reset();

    let mut preroll: VecDeque<Vec<f32>> = VecDeque::with_capacity(preroll_frames);
    let mut collected: Vec<Vec<f32>> = Vec::new();
    let mut speaking = false;
    let mut waited = 0usize;
    let mut silent = 0usize;

    for frame in source {
        let is_speech = vad.is_speech(&frame);

        if !speaking {
            if is_speech {
                speaking = true;
                // include the lead-in
                collected.extend(preroll.drain(..));
                collected.push(frame);
            } else {
                if preroll.len() == preroll_frames {
                    preroll.pop_front();
                }
                preroll.push_back(frame);
                waited += 1;
                if waited as f32 * FRAME_SECONDS >= wait_seconds {
                    return None;
                }
            }
            continue;
        }

        collected.push(frame);
        silent = if is_speech { 0 } else { silent + 1 };

        if silent as f32 * FRAME_SECONDS >= silence_seconds
            || collected.len() as f32 * FRAME_SECONDS >= max_seconds
        {
            break;
        }
    }

    if collected.is_empty() {
        None
    } else {
        Some(collected.into_iter().flatten().collect())
    }
}
