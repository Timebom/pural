use std::collections::HashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use meval::Error as EvalError;

use crate::assistant::interfaces::ToolResult;

// spoken → symbol. Order matters: "to the power of" before "times"
static SPOKEN: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (Regex::new(r"\bto the power of\b|\^").unwrap(), "**"),
        (Regex::new(r"\bdivided by\b|\bover\b").unwrap(), "/"),
        (Regex::new(r"\bmultiplied by\b|\btimes\b").unwrap(), "*"),
        (Regex::new(r"(\d)\s*x\s*(\d)").unwrap(), "$1 * $2"), // still has lookaround → will fix below
        (Regex::new(r"\bplus\b").unwrap(), "+"),
        (Regex::new(r"\bminus\b").unwrap(), "-"),
        (Regex::new(r"\bmodulo\b|\bmod\b").unwrap(), "%"),
    ]
});

// Fixed version without lookaround for the "x" multiplication
static X_AS_MUL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d)\s*x\s*(\d)").unwrap()
});

pub fn calculate(slots: &HashMap<String, String>) -> ToolResult {
    let mut expr = slots.get("expression").cloned().unwrap_or_default();

    // Replace spoken words with operators
    for (pattern, symbol) in SPOKEN.iter() {
        expr = pattern.replace_all(&expr, format!(" {} ", symbol)).into_owned();
    }
    // Handle "5 x 3" → "5 * 3"
    expr = X_AS_MUL.replace_all(&expr, "$1 * $2").into_owned();

    // meval uses `^` for power, not `**`
    expr = expr.replace("**", "^");

    let expr = expr.trim();

    match meval::eval_str(expr) {
        Ok(value) => {
            // Format nicely (same spirit as Python's :.6g)
            let shown = if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{}", value as i64)
            } else {
                format!("{:.6}", value)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            };

            let mut data = HashMap::new();
            data.insert("value".to_string(), value.to_string());

            ToolResult::success(format!("The answer is {}.", shown)).with_data(data)
        }
        Err(EvalError::Function(_, _)) => {
            ToolResult::failure("I can't divide by zero.")
        }
        Err(_) => ToolResult::failure("Sorry, I couldn't work that out."),
    }
}
