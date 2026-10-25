use std::sync::Arc;

use crate::assistant::core::dispatcher::Dispatcher;
use crate::assistant::interfaces::IntentEngine;
use crate::assistant::tools::{calculator, clock, fun};
use crate::assistant::tools::timer::TimerManager;
use crate::assistant::tools::volume::VolumeControl;

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
