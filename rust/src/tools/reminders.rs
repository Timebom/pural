use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::fs;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::core::intents::normalize;
use crate::core::timeparse::{parse_when, split_reminder};
use crate::interfaces::{FollowUp, ToolResult};


const FIELDS: &[&str] = &["id", "task", "due", "status"];

fn pronouns() -> &'static HashMap<&'static str, &'static str> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("my", "your"),
            ("me", "you"),
            ("i", "you"),
            ("mine", "yours"),
            ("myself", "yourself"),
            ("i'm", "you're"),
            ("i'll", "you'll"),
            ("i've", "you've"),
            ("i'd", "you'd"),
        ])
    })
}

fn filler() -> &'static HashSet<&'static str> {
    use std::sync::OnceLock;
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from([
            "the", "my", "a", "an", "that", "about", "to", "reminder", "reminders",
        ])
    })
}

fn all_words() -> &'static HashSet<&'static str> {
    use std::sync::OnceLock;
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| HashSet::from(["all", "every", "everything"]))
}


/// `'call my mom' -> 'call your mom'`
pub fn to_second_person(text: &str) -> String {
    text.split_whitespace()
        .map(|word| pronouns().get(word).copied().unwrap_or(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Human-readable relative time description.
pub fn describe_time(due: NaiveDateTime, now: NaiveDateTime) -> String {
    let hour12 = {
        let h = due.hour() % 12;
        if h == 0 { 12 } else { h }
    };
    let suffix = if due.hour() < 12 { "AM" } else { "PM" };

    let clock = if due.minute() == 0 {
        format!("{hour12} {suffix}")
    } else {
        format!("{hour12}:{:02} {suffix}", due.minute())
    };

    let days = (due.date() - now.date()).num_days();

    match days {
        0 => format!("today at {clock}"),
        1 => format!("tomorrow at {clock}"),
        -1 => format!("yesterday at {clock}"),
        2..=6 => format!("on {} at {clock}", due.format("%A")),
        _ => format!("on {} {} at {clock}", due.format("%B"), due.day()),
    }
}

/// Simple ratio similar to `difflib.SequenceMatcher.ratio()`.
fn sequence_ratio(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Very lightweight approximation: character overlap / max length
    // For production you may prefer the `strsim` crate (Jaro-Winkler etc.)
    let a_chars: HashSet<char> = a.chars().collect();
    let b_chars: HashSet<char> = b.chars().collect();
    let common = a_chars.intersection(&b_chars).count() as f64;
    let total = a.len().max(b.len()) as f64;
    common / total
}

fn matches(query: &str, task: &str) -> bool {
    let task = task.to_lowercase();
    if task.contains(query) {
        return true;
    }
    let task_words: Vec<&str> = task.split_whitespace().collect();
    query
        .split_whitespace()
        .all(|w| task_words.iter().any(|t| sequence_ratio(w, t) >= 0.8))
}

fn now_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
struct Reminder {
    id: String,
    task: String,
    due: f64,          // unix timestamp
    status: String,    // "pending" | "done"
    #[serde(default)]
    created: f64,
}


pub struct ReminderService {
    path: PathBuf,
    on_fire: Arc<dyn Fn(String) + Send + Sync>,
    poll_seconds: f64,
    items: Mutex<Vec<Reminder>>,
    stop: tokio::sync::watch::Sender<bool>,
}

impl ReminderService {
    pub async fn new<P, F>(path: P, on_fire: F, poll_seconds: f64) -> Self
    where
        P: AsRef<Path>,
        F: Fn(String) + Send + Sync + 'static,
    {
        let path = path.as_ref().to_path_buf();
        let items = Self::load(&path).await;

        let (stop_tx, _) = tokio::sync::watch::channel(false);

        Self {
            path,
            on_fire: Arc::new(on_fire),
            poll_seconds,
            items: Mutex::new(items),
            stop: stop_tx,
        }
    }

    async fn load(path: &Path) -> Vec<Reminder> {
        let data = match fs::read_to_string(path).await {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let parsed: Result<Vec<Reminder>, _> = serde_json::from_str(&data);
        let Ok(list) = parsed else {
            // Keep a copy of the corrupt file
            let _ = fs::rename(path, path.with_extension("corrupt")).await;
            return Vec::new();
        };

        let recent = now_timestamp() - 7.0 * 86400.0;
        list.into_iter()
            .filter(|r| r.status == "pending" || r.due >= recent)
            .collect()
    }

    async fn save(&self, items: &[Reminder]) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        let tmp = self.path.with_extension("tmp");
        if let Ok(json) = serde_json::to_string_pretty(items) {
            if fs::write(&tmp, json).await.is_ok() {
                let _ = fs::rename(&tmp, &self.path).await;
            }
        }
    }

    /// Start the background polling loop.
    pub fn start(self: &Arc<Self>) {
        let this = Arc::clone(self);
        let mut stop_rx = self.stop.subscribe();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs_f64(this.poll_seconds));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let _ = this.fire_due(None).await;
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }

    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }

    /// Speak every pending reminder whose time has come. Returns how many fired.
    pub async fn fire_due(&self, now: Option<f64>) -> usize {
        let now = now.unwrap_or_else(now_timestamp);

        let due: Vec<Reminder> = {
            let mut items = self.items.lock().await;
            let due: Vec<_> = items
                .iter_mut()
                .filter(|r| r.status == "pending" && r.due <= now)
                .map(|r| {
                    r.status = "done".to_string();
                    r.clone()
                })
                .collect();

            if !due.is_empty() {
                self.save(&items).await;
            }
            due
        };

        for r in &due {
            (self.on_fire)(Self::fire_message(r, now));
        }

        due.len()
    }

    fn fire_message(r: &Reminder, now: f64) -> String {
        let task = to_second_person(&r.task);
        if now - r.due > 120.0 {
            let due_dt = NaiveDateTime::from_timestamp_opt(r.due as i64, 0)
                .unwrap_or_else(|| Local::now().naive_local());
            let now_dt = NaiveDateTime::from_timestamp_opt(now as i64, 0)
                .unwrap_or_else(|| Local::now().naive_local());
            let when = describe_time(due_dt, now_dt);
            format!("You missed a reminder from {when}: {task}.")
        } else {
            format!("Reminder: {task}.")
        }
    }

    // -----------------------------------------------------------------------
    // Tool methods
    // -----------------------------------------------------------------------

    pub async fn set_reminder(&self, slots: &HashMap<String, String>) -> ToolResult {
        let mut task = slots.get("task").map(|s| s.trim().to_string()).unwrap_or_default();
        let mut when = slots.get("when").map(|s| s.trim().to_string()).unwrap_or_default();

        if let Some(text) = slots.get("text").map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let (t, w) = split_reminder(text, None);
            task = t;
            when = w;
        }

        if task.is_empty() && when.is_empty() {
            return ToolResult::err("What should I remind you about?")
                .with_follow_up(FollowUp::new("set_reminder", "text"));
        }

        if task.is_empty() {
            return ToolResult::err("Remind you to do what?").with_follow_up(
                FollowUp::new("set_reminder", "task").with_slots(HashMap::from([(
                    "when".into(),
                    when,
                )])),
            );
        }

        let ask_when = FollowUp::new("set_reminder", "when")
            .with_slots(HashMap::from([("task".into(), task.clone())]));

        if when.is_empty() {
            return ToolResult::err("When should I remind you?").with_follow_up(ask_when);
        }

        let now = Local::now().naive_local();
        let Some(due) = parse_when(&when, Some(now)) else {
            return ToolResult::err("Sorry, I didn't catch when. When should I remind you?")
                .with_follow_up(ask_when);
        };

        if due <= now {
            return ToolResult::err("That time has already passed. When should I remind you?")
                .with_follow_up(ask_when);
        }

        let entry = Reminder {
            id: Uuid::new_v4().simple().to_string()[..8].to_string(),
            task: task.clone(),
            due: due.and_utc().timestamp() as f64,
            status: "pending".into(),
            created: now_timestamp(),
        };

        {
            let mut items = self.items.lock().await;
            items.push(entry.clone());
            self.save(&items).await;
        }

        let spoken = format!(
            "Okay, I'll remind you to {} {}.",
            to_second_person(&task),
            describe_time(due, now)
        );

        let mut data = HashMap::new();
        data.insert("id".to_string(), entry.id);

        ToolResult::ok(spoken).with_data(data)
    }

    pub async fn list_reminders(&self, _slots: &HashMap<String, String>) -> ToolResult {
        let now = Local::now().naive_local();

        let pending = {
            let items = self.items.lock().await;
            let mut pending: Vec<_> = items
                .iter()
                .filter(|r| r.status == "pending")
                .cloned()
                .collect();
            pending.sort_by(|a, b| a.due.partial_cmp(&b.due).unwrap());
            pending
        };

        if pending.is_empty() {
            return ToolResult::ok("You have no reminders.");
        }

        let spoken: Vec<String> = pending
            .iter()
            .take(5)
            .map(|r| {
                let due_dt = NaiveDateTime::from_timestamp_opt(r.due as i64, 0)
                    .unwrap_or(now);
                format!(
                    "{} {}",
                    to_second_person(&r.task),
                    describe_time(due_dt, now)
                )
            })
            .collect();

        let intro = if pending.len() == 1 {
            "You have one reminder: ".to_string()
        } else {
            format!("You have {} reminders: ", pending.len())
        };

        let mut message = intro + &spoken.join(". ") + ".";
        if pending.len() > 5 {
            message += &format!(" And {} more.", pending.len() - 5);
        }

        let mut data = HashMap::new();
        data.insert("count".to_string(), pending.len().to_string());

        ToolResult::ok(message).with_data(data)
    }

    pub async fn delete_reminder(&self, slots: &HashMap<String, String>) -> ToolResult {
        let words: Vec<&str> = normalize(slots.get("task").map(|s| s.as_str()).unwrap_or(""))
            .split_whitespace()
            .filter(|w| !filler().contains(w))
            .collect();

        let query = words.join(" ");
        if query.is_empty() {
            return ToolResult::err("Which reminder?")
                .with_follow_up(FollowUp::new("delete_reminder", "task"));
        }

        let (matches, message) = {
            let mut items = self.items.lock().await;
            let pending: Vec<_> = items
                .iter()
                .filter(|r| r.status == "pending")
                .cloned()
                .collect();

            let matches: Vec<_> = if all_words().contains(query.as_str()) {
                pending
            } else {
                pending
                    .into_iter()
                    .filter(|r| matches(&query, &r.task))
                    .collect()
            };

            if matches.is_empty() {
                return ToolResult::err(format!("I couldn't find a reminder about {query}."));
            }

            if matches.len() > 1 && !all_words().contains(query.as_str()) {
                let names = matches
                    .iter()
                    .take(3)
                    .map(|r| to_second_person(&r.task))
                    .collect::<Vec<_>>()
                    .join(", and ");
                return ToolResult::err(format!(
                    "That matches {} reminders: {names}. Which one?",
                    matches.len()
                ))
                .with_follow_up(FollowUp::new("delete_reminder", "task"));
            }

            let ids: HashSet<_> = matches.iter().map(|r| r.id.clone()).collect();
            items.retain(|r| !ids.contains(&r.id));
            self.save(&items).await;

            let msg = if all_words().contains(query.as_str()) {
                format!("Okay, I cleared {} reminders.", matches.len())
            } else {
                format!(
                    "Okay, I deleted the reminder to {}.",
                    to_second_person(&matches[0].task)
                )
            };

            (matches, msg)
        };

        ToolResult::ok(message)
    }
}
