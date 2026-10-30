use std::collections::HashMap;

use crate::interfaces::ToolResult;

/// Speaks an answer the LLM already wrote (the `chat_reply` pseudo-intent).
pub fn chat_reply(slots: &HashMap<String, String>) -> ToolResult {
    let text = slots
        .get("text")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    match text {
        Some(t) => ToolResult::ok(t),
        None => ToolResult::err("Sorry, I don't have an answer for that."),
    }
}
