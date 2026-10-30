use chrono::{Local, Datelike, Timelike};
use std::collections::HashMap;
use crate::assistant::interfaces::ToolResult;

pub fn get_time(_slots: &HashMap<String, String>) -> ToolResult {
    let now = Local::now();

    let hour12 = {
        let h = now.hour() % 12;
        if h == 0 { 12 } else { h }
    };

    let suffix = if now.hour() < 12 { "AM" } else { "PM" };
    let spoken = format!("{}:{:02} {}", hour12, now.minute(), suffix);

    let mut data = HashMap::new();
    data.insert("iso".to_string(), now.to_rfc3339());

    ToolResult::success(format!("It's {}.", spoken)).with_data(data)
}

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
    data.insert("iso".to_string(), now.format("%Y-%m-%d").to_string());

    ToolResult::success(format!("Today is {}.", spoken)).with_data(data)
}
