use std::sync::Arc;
use std::time::Instant;

use crate::assistant::core::dispatcher::Dispatcher;
use crate::assistant::interfaces::{IntentEngine, STT};
use crate::assistant::tools::{calculator, clock, fun};
use crate::assistant::tools::timer::TimerManager;
use crate::assistant::tools::volume::VolumeControl;
use crate::assistant::vad::{record_until_silence, SileroVAD};
use crate::assistant::audio::frames;

pub fn build_dispatcher<F>(notify: F) -> Dispatcher
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut d = Dispatcher::new();

    let timers = Arc::new(TimerManager::new(notify));
    let volume = Arc::new(VolumeControl::new());

    d.register("get_time", clock::get_time);
    d.register("get_date", clock::get_date);

    // Timer
    {
        let timers = Arc::clone(&timers);
        d.register("set_timer", move |slots| timers.set_timer(slots));
    }

    // Calculator & jokes
    d.register("calculate", calculator::calculate);
    d.register("tell_joke", fun::tell_joke);

    // Volume
    {
        let volume = Arc::clone(&volume);
        d.register("set_volume", move |slots| volume.set_volume(slots));
    }
    {
        let volume = Arc::clone(&volume);
        d.register("volume_up", move |slots| volume.volume_up(slots));
    }
    {
        let volume = Arc::clone(&volume);
        d.register("volume_down", move |slots| volume.volume_down(slots));
    }

    d
}

/// One full turn: text in, speakable reply out.
pub fn handle(text: &str, engine: &impl IntentEngine, dispatcher: &Dispatcher) -> String {
    match engine.parse(text) {
        Some(intent) => dispatcher.run(&intent).message,
        None => "Sorry, I didn't catch that.".to_string(),
    }
}

/// Capture one utterance from the mic and return its text (`""` if nobody spoke).
pub fn listen_once(vad: &mut SileroVAD, stt: &impl STT) -> String {
    let mut stream = match frames(None) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to open mic: {e}");
            return String::new();
        }
    };

    // Stream is dropped at end of scope → mic is released
    let clip = record_until_silence(
        vad,
        std::iter::from_fn(|| stream.next()),
        5.0,  // wait_seconds
        0.8,  // silence_seconds
        15.0, // max_seconds
        4,    // preroll_frames
    );

    let Some(clip) = clip else {
        return String::new();
    };

    let start = Instant::now();
    let text = stt.transcribe(&clip);
    let elapsed = start.elapsed().as_secs_f32();
    let audio_secs = clip.len() as f32 / 16_000.0;

    log::info!(
        "transcribed {:.1} seconds of audio in {:.2} seconds: {:?}",
        audio_secs,
        elapsed,
        text
    );

    text
}
