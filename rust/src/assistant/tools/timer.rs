use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::sync::mpsc::{self, Sender};

use crate::assistant::core::parsing::parse_duration;
use crate::assistant::interfaces::ToolResult;

const MAX_SECONDS: i64 = 24 * 3600;

/// 90 → "1 minute 30 seconds"
pub fn format_duration(seconds: i64) -> String {
    let mut seconds = seconds;
    let mut parts = Vec::new();

    for (unit, size) in [("hour", 3600), ("minute", 60), ("second", 1)] {
        let count = seconds / size;
        seconds %= size;

        if count > 0 {
            let plural = if count != 1 { "s" } else { "" };
            parts.push(format!("{} {}{}", count, unit, plural));
        }
    }

    parts.join(" ")
}

struct ActiveTimer {
    cancel_tx: Sender<()>,
    handle: thread::JoinHandle<()>,
}

/// Owns running timers. `on_fire` is how a finished timer reaches the user.
pub struct TimerManager {
    on_fire: Arc<dyn Fn(String) + Send + Sync>,
    timers: Arc<Mutex<Vec<ActiveTimer>>>,
}

impl TimerManager {
    pub fn new<F>(on_fire: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        Self {
            on_fire: Arc::new(on_fire),
            timers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn set_timer(&self, slots: &HashMap<String, String>) -> ToolResult {
        let raw = match slots.get("duration") {
            Some(v) => v,
            None => return ToolResult::failure("For how long?"),
        };

        let seconds = match parse_duration(raw) {
            Some(s) => s,
            None => return ToolResult::failure("Sorry, I didn't catch how long."),
        };

        if seconds > MAX_SECONDS {
            return ToolResult::failure("That's longer than I can time. The limit is 24 hours.");
        }

        let label = format_duration(seconds);
        let on_fire = Arc::clone(&self.on_fire);
        let timers = Arc::clone(&self.timers);

        // Channel used to cancel this specific timer
        let (cancel_tx, cancel_rx) = mpsc::channel::<()>();

        let value = label.clone();
        let handle = thread::spawn(move || {
            // Wait either for the duration or a cancel signal
            let cancelled = cancel_rx
                .recv_timeout(Duration::from_secs(seconds as u64))
                .is_ok(); // Ok means we received the cancel message

            if cancelled {
                return; // timer was cancelled → do nothing
            }

            // Timer finished normally
            on_fire(format!("Your timer for {} is done.", value));

            // Best-effort cleanup of finished timers
            if let Ok(mut list) = timers.lock() {
                list.retain(|t| !t.handle.is_finished());
            }
        });

        if let Ok(mut list) = self.timers.lock() {
            list.push(ActiveTimer { cancel_tx, handle });
        }

        let mut data = HashMap::new();
        data.insert("seconds".to_string(), seconds.to_string());

        ToolResult::success(format!("Timer set for {}.", label)).with_data(data)
    }

    /// Cancel all currently running timers.
    pub fn cancel_all(&self) {
        if let Ok(mut list) = self.timers.lock() {
            for timer in list.drain(..) {
                // Send cancel signal (ignore if the receiver is already gone)
                let _ = timer.cancel_tx.send(());
                // We don't join here to avoid blocking the main thread
            }
        }
    }
}
