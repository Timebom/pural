use regex::Regex;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static UNITS: Lazy<HashMap<&'static str, f64>> = Lazy::new(|| {
    HashMap::from([
        ("zero", 0.0),
        ("one", 1.0),
        ("two", 2.0),
        ("three", 3.0),
        ("four", 4.0),
        ("five", 5.0),
        ("six", 6.0),
        ("seven", 7.0),
        ("eight", 8.0),
        ("nine", 9.0),
        ("ten", 10.0),
        ("eleven", 11.0),
        ("twelve", 12.0),
        ("thirteen", 13.0),
        ("fourteen", 14.0),
        ("fifteen", 15.0),
        ("sixteen", 16.0),
        ("seventeen", 17.0),
        ("eighteen", 18.0),
        ("nineteen", 19.0),
    ])
});

static TENS: Lazy<HashMap<&'static str, f64>> = Lazy::new(|| {
    HashMap::from([
        ("twenty", 20.0),
        ("thirty", 30.0),
        ("forty", 40.0),
        ("fifty", 50.0),
        ("sixty", 60.0),
        ("seventy", 70.0),
        ("eighty", 80.0),
        ("ninety", 90.0),
    ])
});

static SECONDS_PER: Lazy<HashMap<&'static str, f64>> = Lazy::new(|| {
    HashMap::from([
        ("second", 1.0),
        ("seconds", 1.0),
        ("sec", 1.0),
        ("secs", 1.0),
        ("minute", 60.0),
        ("minutes", 60.0),
        ("min", 60.0),
        ("mins", 60.0),
        ("hour", 3600.0),
        ("hours", 3600.0),
        ("hr", 3600.0),
        ("hrs", 3600.0),
    ])
});

static DIGITS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d+(?:\.\d+)?$").unwrap());

/// `"5"` → `Some(5.0)`, `"2.5"` → `Some(2.5)`, `"five"` → `Some(5.0)`, etc.
fn as_number(token: &str) -> Option<f64> {
    if let Some(&v) = UNITS.get(token) {
        return Some(v);
    }
    if let Some(&v) = TENS.get(token) {
        return Some(v);
    }
    if DIGITS.is_match(token) {
        return token.parse::<f64>().ok();
    }
    None
}

/// `"two and a half hours"` → `Some(9000)`.
/// Returns seconds, or `None` if no duration found.
pub fn parse_duration(text: &str) -> Option<i64> {
    let binding = text
        .to_lowercase()
        .replace("-", " ");
    let tokens: Vec<&str> = binding
        .split_whitespace()
        .collect();

    let mut total = 0.0_f64;
    let mut number: Option<f64> = None;
    let mut found_unit = false;

    for (i, tok) in tokens.iter().enumerate() {
        if let Some(&secs) = SECONDS_PER.get(tok) {
            let amount = number.unwrap_or(1.0);
            total += amount * secs;
            number = None;
            found_unit = true;
        } else if *tok == "half" {
            number = Some(number.unwrap_or(0.0) + 0.5);
        } else if *tok == "a" || *tok == "an" {
            // "an hour" → 1. But before "half", stay out of the way:
            // the "half" branch adds the 0.5 itself.
            let next_is_half = tokens.get(i + 1).map(|t| *t == "half").unwrap_or(false);
            if number.is_none() && !next_is_half {
                number = Some(1.0);
            }
        } else if let Some(value) = as_number(tok) {
            let is_tens_then_unit = number
                .map(|n| n >= 20.0 && n % 10.0 == 0.0 && value < 10.0 && UNITS.contains_key(tok))
                .unwrap_or(false);

            number = if is_tens_then_unit {
                Some(number.unwrap() + value)
            } else {
                Some(value)
            };
        }
    }

    if found_unit && total > 0.0 {
        Some(total.round() as i64)
    } else {
        None
    }
}
