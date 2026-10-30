use crate::assistant::audio::{frames, SAMPLE_RATE};
use crate::assistant::core::dispatcher::Dispatcher;
use crate::assistant::interfaces::STT;
use crate::assistant::tools::{calculator, clock, fun};
use crate::assistant::tools::timer::TimerManager;
use crate::assistant::tools::volume::{get_backend, VolumeControl};
use crate::assistant::vad::{record_until_silence, SileroVAD};
use crate::assistant::wakeword::{wait_for_wakeword, WakeWordDetector};

use log::info;
use std::time::Instant;

const SETTLE_FRAMES: usize = 3;

/// Build a fully-populated dispatcher with all the built-in tools.
pub fn build_dispatcher(
    notify: impl Fn(String) + Send + Sync + 'static,
) -> Dispatcher {
    let mut d = Dispatcher::new();

    let timers = TimerManager::new(notify);
    let volume = VolumeControl::new(get_backend());

    d.register("get_time", clock::get_time);
    d.register("get_date", clock::get_date);
    d.register("set_timer", move |slots| timers.set_timer(slots));
    d.register("calculate", calculator::calculate);
    d.register("tell_joke", fun::tell_joke);
    d.register("set_volume", move |slots| volume.set_volume(slots));
    d.register("volume_up", move |slots| volume.volume_up(slots));
    d.register("volume_down", move |slots| volume.volume_down(slots));

    d
}

fn transcribe(clip: &[f32], stt: &impl STT) -> String {
    let start = Instant::now();
    let text = stt.transcribe(clip);
    let duration_s = clip.len() as f64 / SAMPLE_RATE as f64;
    info!(
        "transcribed {:.1} seconds of audio in {:.2} seconds: {:?}",
        duration_s,
        start.elapsed().as_secs_f64(),
        text
    );
    text
}

/// Capture one utterance from the mic and return its text
/// (`""` if nobody spoke).
pub fn listen_once(vad: &mut SileroVAD, stt: &impl STT) -> String {
    // `frames()` owns the cpal stream; dropping it releases the mic.
    let source = match frames(None) {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to open mic: {e}");
            return String::new();
        }
    };

    let clip = record_until_silence(vad, source);

    match clip {
        None => String::new(),
        Some(samples) => transcribe(&samples, stt),
    }
}

/// Block until the wake phrase, then capture one command on the same mic stream.
pub fn wake_turn(
    detector: &mut WakeWordDetector,
    vad: &mut SileroVAD,
    stt: &impl STT,
    on_wake: impl FnOnce(),
) -> String {
    detector.reset();

    let mut source = match frames(None) {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to open mic: {e}");
            return String::new();
        }
    };

    if !wait_for_wakeword(detector, &mut source) {
        return String::new();
    }

    on_wake();

    // Discard a few frames so the wake-word itself is not transcribed
    for _ in source.by_ref().take(SETTLE_FRAMES) {}

    let clip = record_until_silence(vad, source);

    match clip {
        None => String::new(),
        Some(samples) => transcribe(&samples, stt),
    }
}
