use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use rand::prelude::IndexedRandom;

use crate::assistant::interfaces::ToolResult;

static JOKES: &[&str] = &[
    "Why do programmers prefer dark mode? Because light attracts bugs.",
    "I would tell you a UDP joke, but you might not get it.",
    "There are only 10 kinds of people: those who understand binary and those who don't.",
    "Why did the developer go broke? Because he used up all his cache.",
    "A SQL query walks into a bar, walks up to two tables and asks: can I join you?",
];

static LAST: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

pub fn tell_joke(_slots: &HashMap<String, String>) -> ToolResult {
    let mut last = LAST.lock().unwrap();

    let choices: Vec<&str> = JOKES
        .iter()
        .copied()
        .filter(|j| Some(*j) != last.as_deref())
        .collect();

    let selected = choices
        .choose(&mut rand::rng())
        .copied()
        .unwrap_or(JOKES[0]);

    *last = Some(selected.to_string());

    ToolResult::success(selected)
}
