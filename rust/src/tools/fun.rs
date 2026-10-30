use std::collections::HashMap;
use std::sync::Mutex;

use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::interfaces::ToolResult;

const JOKES: &[&str] = &[
    "Why do programmers prefer dark mode? Because light attracts bugs.",
    "I would tell you a UDP joke, but you might not get it.",
    "There are only 10 kinds of people: those who understand binary and those who don't.",
    "Why did the developer go broke? Because he used up all his cache.",
    "A SQL query walks into a bar, walks up to two tables and asks: can I join you?",
];

/// Remembers the last joke so we don't repeat it immediately.
static LAST_JOKE: Mutex<Option<String>> = Mutex::new(None);

pub fn tell_joke(_slots: &HashMap<String, String>) -> ToolResult {
    let mut last = LAST_JOKE.lock().unwrap();

    let choices: Vec<&str> = JOKES
        .iter()
        .copied()
        .filter(|j| Some(*j) != last.as_deref())
        .collect();

    // Fallback in the extremely unlikely case every joke is filtered out
    let selected = choices
        .choose(&mut thread_rng())
        .copied()
        .unwrap_or(JOKES[0]);

    *last = Some(selected.to_string());

    ToolResult::ok(selected)
}
