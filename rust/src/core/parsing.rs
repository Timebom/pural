use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

fn units() -> &'static HashMap<&'static str, i32> {
    static MAP: OnceLock<HashMap<&'static str, i32>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("zero", 0),
            ("one", 1),
            ("two", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("six", 6),
            ("seven", 7),
            ("eight", 8),
            ("nine", 9),
            ("ten", 10),
            ("eleven", 11),
            ("twelve", 12),
            ("thirteen", 13),
            ("fourteen", 14),
            ("fifteen", 15),
            ("sixteen", 16),
            ("seventeen", 17),
            ("eighteen", 18),
            ("nineteen", 19),
        ])
    })
}

fn tens() -> &'static HashMap<&'static str, i32> {
    static MAP: OnceLock<HashMap<&'static str, i32>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("twenty", 20),
            ("thirty", 30),
            ("forty", 40),
            ("fifty", 50),
            ("sixty", 60),
            ("seventy", 70),
            ("eighty", 80),
            ("ninety", 90),
        ])
    })
}

fn seconds_per() -> &'static HashMap<&'static str, i32> {
    static MAP: OnceLock<HashMap<&'static str, i32>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("second", 1),
            ("seconds", 1),
            ("sec", 1),
            ("secs", 1),
            ("minute", 60),
            ("minutes", 60),
            ("min", 60),
            ("mins", 60),
            ("hour", 3600),
            ("hours", 3600),
            ("hr", 3600),
            ("hrs", 3600),
        ])
    })
}

fn number_words() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s: HashSet<&'static str> = units().keys().copied().collect();
        s.extend(tens().keys().copied());
        s.insert("hundred");
        s
    })
}

fn digits_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+(?:\.\d+)?$").unwrap())
}

fn hyphen_between_letters() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?<=[a-z])-(?=[a-z])").unwrap())
}

/// Read one number (0-999) at `tokens[i]`.
/// Returns `(value, index_after)` or `None`.
fn read_number(tokens: &[&str], i: usize) -> Option<(i32, usize)> {
    let at = |k: usize| -> &str {
        if k < tokens.len() {
            tokens[k]
        } else {
            ""
        }
    };

    let below_100 = |k: usize| -> Option<(i32, usize)> {
        let word = at(k);
        if let Some(&t) = tens().get(word) {
            let unit = units().get(at(k + 1)).copied().unwrap_or(0);
            if (1..=9).contains(&unit) {
                return Some((t + unit, k + 2)); // "forty five"
            }
            return Some((t, k + 1));
        }
        if let Some(&u) = units().get(word) {
            return Some((u, k + 1));
        }
        if digits_re().is_match(word) {
            let v = word.parse::<f64>().ok()?.round() as i32;
            return Some((v, k + 1));
        }
        None
    };

    let first = below_100(i);

    let (value, j) = if let Some((v, next)) = first {
        if v < 10 && at(next) == "hundred" {
            // "two hundred ..."
            (v * 100, next + 1)
        } else {
            // plain 0-99
            return first;
        }
    } else if (at(i) == "a" || at(i) == "an") && at(i + 1) == "hundred" {
        // "a hundred ..."
        (100, i + 2)
    } else if at(i) == "hundred" {
        // "hundred ..."
        (100, i + 1)
    } else {
        return None;
    };

    // optional "and"
    let k = if at(j) == "and" { j + 1 } else { j };
    if let Some((rest, end)) = below_100(k) {
        if rest < 100 {
            return Some((value + rest, end));
        }
    }
    // don't consume a dangling "and"
    Some((value, j))
}

/// `'5' -> 5.0`, `'2.5' -> 2.5`, `'five' -> 5.0`, `'twenty' -> 20.0`, else `None`
fn as_number(token: &str) -> Option<f64> {
    if let Some(&u) = units().get(token) {
        return Some(u as f64);
    }
    if let Some(&t) = tens().get(token) {
        return Some(t as f64);
    }
    if digits_re().is_match(token) {
        return token.parse::<f64>().ok();
    }
    None
}

/// `"40" -> 40`, `"forty five" -> 45`, `"a hundred" -> 100`, `"40%" -> 40`.
/// Finds the first number in the text. Returns `None` if there isn't one.
pub fn parse_number(text: &str) -> Option<i32> {
    let cleaned = text.to_lowercase().replace('-', " ").replace('%', " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();

    for i in 0..tokens.len() {
        if let Some((value, _)) = read_number(&tokens, i) {
            return Some(value);
        }
    }
    None
}

/// `"seven plus nine" -> "7 plus 9"`.
/// Digits, symbols and minus signs are left alone.
pub fn words_to_digits(text: &str) -> String {
    let lowered = text.to_lowercase();
    let cleaned = hyphen_between_letters().replace_all(&lowered, " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();

    let mut out = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let found = if number_words().contains(tokens[i])
            || tokens[i] == "a"
            || tokens[i] == "an"
        {
            read_number(&tokens, i)
        } else {
            None
        };

        if let Some((value, next)) = found {
            out.push(value.to_string());
            i = next; // skip exactly the tokens the number used
        } else {
            out.push(tokens[i].to_string());
            i += 1;
        }
    }

    out.join(" ")
}

/// `"two and a half hours" -> 9000`.
/// Returns seconds, or `None` if no duration found.
pub fn parse_duration(text: &str) -> Option<i32> {
    let cleaned = text.to_lowercase().replace('-', " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();

    let mut total = 0.0_f64;
    let mut number: Option<f64> = None;
    let mut found_unit = false;

    for (i, tok) in tokens.iter().enumerate() {
        if let Some(&secs) = seconds_per().get(tok) {
            let n = number.unwrap_or(1.0);
            total += n * secs as f64;
            number = None;
            found_unit = true;
        } else if *tok == "half" {
            number = Some(number.unwrap_or(0.0) + 0.5);
        } else if *tok == "a" || *tok == "an" {
            // "an hour" -> 1. But before "half", stay out of the way.
            let next_is_half = tokens.get(i + 1).map(|t| *t == "half").unwrap_or(false);
            if number.is_none() && !next_is_half {
                number = Some(1.0);
            }
        } else if let Some(value) = as_number(tok) {
            let is_tens_then_unit = number
                .map(|n| n >= 20.0 && (n as i32) % 10 == 0 && value < 10.0)
                .unwrap_or(false)
                && units().contains_key(tok);

            number = if is_tens_then_unit {
                Some(number.unwrap() + value)
            } else {
                Some(value)
            };
        }
        // else: ignore the token
    }

    if found_unit && total > 0.0 {
        Some(total.round() as i32)
    } else {
        None
    }
}
