use chrono::{Datelike, Duration, Local, NaiveDateTime, NaiveTime, Timelike};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::core::intents::normalize;
use crate::core::parsing::{NUMBER_WORDS, parse_duration, words_to_digits};


fn days() -> &'static HashMap<&'static str, i64> {
    static MAP: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    MAP.get_or_init(|| HashMap::from([("today", 0), ("tomorrow", 1)]))
}

fn markers() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| HashSet::from(["in", "at", "today", "tomorrow"]))
}

fn trailing() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from(["by", "at", "on", "for", "around", "about"])
    })
}

fn duration_words() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s: HashSet<&'static str> = HashSet::from([
            "second", "seconds", "sec", "secs", "minute", "minutes", "min", "mins", "hour",
            "hours", "hr", "hrs", "and", "a", "an", "half",
        ]);
        s.extend(NUMBER_WORDS.iter().copied());
        s
    })
}

fn number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+(?:\.\d+)?$").unwrap())
}

fn clock_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^(?:(?P<noon>noon|midday)|(?P<midnight>midnight)|(?P<h>\d{1,2})(?:[: ](?P<m>\d{2}))?\s*(?P<ap>a ?m|p ?m)?)$",
        )
        .unwrap()
    })
}

const MAX_AHEAD_DAYS: i64 = 365;


/// `'20 minutes' -> now + 20 min`. Every word must be part of a duration.
fn relative(tokens: &[&str], now: NaiveDateTime) -> Option<NaiveDateTime> {
    if tokens.is_empty() {
        return None;
    }
    if !tokens
        .iter()
        .all(|t| duration_words().contains(t) || number_re().is_match(t))
    {
        return None;
    }

    let seconds = parse_duration(&tokens.join(" "))?;
    let delta = Duration::seconds(seconds as i64);
    if delta > Duration::days(MAX_AHEAD_DAYS) {
        return None;
    }
    Some(now + delta)
}

fn clock(tokens: &[&str], now: NaiveDateTime) -> Option<NaiveDateTime> {
    let mut tokens = tokens.to_vec();

    // leading day
    let mut day = if !tokens.is_empty() && days().contains_key(tokens[0]) {
        Some(tokens.remove(0))
    } else {
        None
    };

    if !tokens.is_empty() && tokens[0] == "at" {
        tokens.remove(0);
    }

    // trailing day
    if day.is_none() && !tokens.is_empty() && days().contains_key(tokens[tokens.len() - 1]) {
        day = Some(tokens.pop().unwrap());
    }

    let joined = tokens.join(" ");
    let caps = clock_re().captures(&joined)?;

    let meridiem = caps
        .name("ap")
        .map(|m| m.as_str().replace(' ', ""))
        .unwrap_or_default();

    let (hour, minute, exact) = if caps.name("noon").is_some() {
        (12u32, 0u32, true)
    } else if caps.name("midnight").is_some() {
        (0u32, 0u32, true)
    } else {
        let h: u32 = caps.name("h")?.as_str().parse().ok()?;
        let m: u32 = caps
            .name("m")
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);

        if m > 59 || h > 23 || (!meridiem.is_empty() && !(1..=12).contains(&h)) {
            return None;
        }

        let hour = if !meridiem.is_empty() {
            h % 12 + if meridiem == "pm" { 12 } else { 0 }
        } else {
            h
        };

        // False: "5" could be AM or PM
        let exact = !meridiem.is_empty() || hour == 0 || hour >= 13;
        (hour, m, exact)
    };

    if let Some(d) = day {
        let mut hour = hour;
        // "tomorrow at 5" means 5 PM; 7 to 11 stay AM; 12 is noon
        if !exact && hour != 12 && hour <= 6 {
            hour += 12;
        }
        let days_offset = *days().get(d)?;
        let date = now.date() + Duration::days(days_offset);
        return NaiveDateTime::new(date, NaiveTime::from_hms_opt(hour, minute, 0)?).into();
    }

    // no explicit day → try today then tomorrow
    let options = if exact {
        vec![hour]
    } else {
        vec![hour % 12, hour % 12 + 12]
    };

    for offset in 0..=1 {
        let date = now.date() + Duration::days(offset);
        let mut upcoming = Vec::new();

        for &h in &options {
            if let Some(t) = NaiveTime::from_hms_opt(h, minute, 0) {
                let candidate = NaiveDateTime::new(date, t);
                if candidate > now {
                    upcoming.push(candidate);
                }
            }
        }

        if let Some(earliest) = upcoming.into_iter().min() {
            return Some(earliest);
        }
    }

    None
}


/// `'in 20 minutes'`, `'at 5:30 pm'`, `'tomorrow at nine'` → `NaiveDateTime`.
/// Returns `None` if the text is not a pure time phrase.
///
/// The result is in the past only when the user said `"today"` with a time
/// that has already gone by.
pub fn parse_when(text: &str, now: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    let now = now.unwrap_or_else(|| Local::now().naive_local());
    let tokens: Vec<&str> = words_to_digits(&normalize(text))
        .split_whitespace()
        .collect();

    if tokens.is_empty() {
        return None;
    }

    if tokens[0] == "in" {
        return relative(&tokens[1..], now);
    }
    clock(&tokens, now)
}

/// `'call mom tomorrow at 9'` → `("call mom", "tomorrow at 9")`.
/// Missing parts are empty strings.
pub fn split_reminder(text: &str, now: Option<NaiveDateTime>) -> (String, String) {
    let now = now.unwrap_or_else(|| Local::now().naive_local());
    let tokens: Vec<&str> = normalize(text).split_whitespace().collect();

    // Time first: "in twenty minutes to check the oven"
    if !tokens.is_empty() && markers().contains(tokens[0]) {
        for j in 1..tokens.len() {
            if tokens[j] == "to" {
                let when_part = tokens[..j].join(" ");
                if parse_when(&when_part, Some(now)).is_some() {
                    let task = tokens[j + 1..].join(" ");
                    return (task, when_part);
                }
            }
        }
    }

    // Time last: "check the oven in twenty minutes". Earliest valid split wins.
    for i in 0..tokens.len() {
        if markers().contains(tokens[i]) {
            let when_part = tokens[i..].join(" ");
            if parse_when(&when_part, Some(now)).is_some() {
                let mut task: Vec<&str> = tokens[..i].to_vec();
                while let Some(last) = task.last() {
                    if trailing().contains(last) {
                        task.pop();
                    } else {
                        break;
                    }
                }
                return (task.join(" "), when_part);
            }
        }
    }

    (tokens.join(" "), String::new())
}
