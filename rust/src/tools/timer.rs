use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use tokio::time::{sleep, Duration};

use crate::core::parsing::parse_duration;
use crate::interfaces::{FollowUp, ToolResult};

const MAX_SECONDS: i32 = 24 * 3600;

/// `90 -> "1 minute 30 seconds"`
pub fn format_duration(mut seconds: i32) -> String {
    let mut parts = Vec::new();

    for (unit, size) in [("hour", 3600), ("minute", 60), ("second", 1)] {
        let count = seconds / size;
        seconds %= size;
        if count > 0 {
            let plural = if count != 1 { "s" } else { "" };
            parts.push(format!("{count} {unit}{plural}"));
        }
    }

    parts.join(" ")
}

/// Owns running timers. `on_fire` is how a finished timer reaches the user.
pub struct TimerManager {
    on_fire: Arc<dyn Fn(String) + Send + Sync>,
    active: Mutex<HashMap<u64, AbortHandle>>,
    next_id: AtomicU64,
}

impl TimerManager {
    pub fn new<F>(on_fire: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        Self {
            on_fire: Arc::new(on_fire),
            active: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    pub async fn set_timer(&self, slots: &HashMap<String, String>) -> ToolResult {
        let Some(raw) = slots.get("duration") else {
            return ToolResult::err("For how long?")
                .with_follow_up(FollowUp::new("set_timer", "duration"));
        };

        let Some(seconds) = parse_duration(raw) else {
            return ToolResult::err("Sorry, I didn't catch how long. For how long?")
                .with_follow_up(FollowUp::new("set_timer", "duration"));
        };

        if seconds > MAX_SECONDS {
            return ToolResult::err("That's longer than I can time. The limit is 24 hours.");
        }

        let label = format_duration(seconds);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let on_fire = Arc::clone(&self.on_fire);
        let label_for_task = label.clone();

        // Spawn the timer task
        let handle = tokio::spawn(async move {
            sleep(Duration::from_secs(seconds as u64)).await;
            on_fire(format!("Your timer for {label_for_task} is done."));
        });

        // Store the AbortHandle so we can cancel later
        self.active.lock().await.insert(id, handle.abort_handle());

        // When the task finishes (or is aborted) we can optionally clean up,
        // but for simplicity we leave the entry; cancel_all will clear everything.

        let mut data = HashMap::new();
        data.insert("seconds".to_string(), seconds.to_string());

        ToolResult::ok(format!("Timer set for {label}."))
            .with_data(data)
    }

    /// Cancel all running timers.
    pub async fn cancel_all(&self) {
        let mut map = self.active.lock().await;
        for (_, handle) in map.drain() {
            handle.abort();
        }
    }
}
