use std::collections::HashMap;

use evalexpr::{eval_number, EvalexprError};
use regex::Regex;
use std::sync::OnceLock;

use crate::core::parsing::words_to_digits;
use crate::interfaces::ToolResult;

// Spoken → symbol. Order matters: "to the power of" before "times"
fn spoken_replacements() -> &'static Vec<(Regex, &'static str)> {
    static LIST: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    LIST.get_or_init(|| {
        vec![
            (
                Regex::new(r"\bto the power of\b|\^").unwrap(),
                "**",
            ),
            (
                Regex::new(r"\bdivided by\b|\bover\b").unwrap(),
                "/",
            ),
            (
                Regex::new(r"\bmultiplied by\b|\btimes\b").unwrap(),
                "*",
            ),
            (
                Regex::new(r"(\d)\s*x\s*(\d)").unwrap(),
                "$1 * $2",
            ),
            (Regex::new(r"\bplus\b").unwrap(), "+"),
            (Regex::new(r"\bminus\b").unwrap(), "-"),
            (
                Regex::new(r"\bmodulo\b|\bmod\b").unwrap(),
                "%",
            ),
        ]
    })
}

pub fn calculate(slots: &HashMap<String, String>) -> ToolResult {
    let mut expr = slots
        .get("expression")
        .cloned()
        .unwrap_or_default();

    expr = words_to_digits(&expr);

    for (pattern, symbol) in spoken_replacements() {
        expr = pattern.replace_all(&expr, format!(" {symbol} ")).into_owned();
    }

    let expr = expr.trim();

    // evalexpr uses `^` for power, so convert `**` → `^`
    let expr = expr.replace("**", "^");

    match eval_number(&expr) {
        Ok(value) => {
            // Guard against huge exponents / overflow that slipped through
            if !value.is_finite() {
                return ToolResult::err("Sorry, I couldn't work that out.");
            }

            let shown = format!("{value:.6}");
            // trim trailing zeros / decimal point for nicer speech
            let shown = shown
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();

            let mut data = HashMap::new();
            data.insert("value".to_string(), value.to_string());

            ToolResult::ok(format!("The answer is {shown}."))
                .with_data(data)
        }
        Err(EvalexprError::DivisionError) | Err(EvalexprError::ModuloError) => {
            ToolResult::err("I can't divide by zero.")
        }
        Err(_) => ToolResult::err("Sorry, I couldn't work that out."),
    }
}
