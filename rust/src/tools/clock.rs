use chrono::Local;
use std::collections::HashMap;

use crate::interfaces::ToolResult;

/// Current local time in 12-hour format.
pub fn get_time(_slots: &HashMap<String, String>) -> ToolResult {
    let now = Local::now();
    let hour12 = {
        let h = now.hour() % 12;
        if h == 0 { 12 } else { h }
    };
    let suffix = if now.hour() < 12 { "AM" } else { "PM" };
    let spoken = format!("{hour12}:{:02} {suffix}", now.minute());

    let mut data = HashMap::new();
    data.insert("iso".to_string(), now.to_rfc3339());

    ToolResult::ok(format!("It's {spoken}."))
        .with_data(data)
}

/// Current local date in a spoken form.
pub fn get_date(_slots: &HashMap<String, String>) -> ToolResult {
    let now = Local::now();
    let spoken = format!(
        "{}, {} {}, {}",
        now.format("%A"),   // weekday
        now.format("%B"),   // month name
        now.day(),
        now.year()
    );

    let mut data = HashMap::new();
    data.insert("iso".to_string(), now.date_naive().to_string()); // YYYY-MM-DD

    ToolResult::ok(format!("Today is {spoken}."))
        .with_data(data)
}
