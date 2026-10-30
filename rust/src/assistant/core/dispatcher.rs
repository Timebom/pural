use std::collections::HashMap;
use crate::assistant::interfaces::{Intent, ToolResult};

/// Type alias for a tool function.
/// Takes the intent slots and returns a ToolResult.
pub type ToolFn = Box<dyn Fn(&HashMap<String, String>) -> ToolResult + Send + Sync>;

/// Maps Intent.name to a tool function and never lets a tool crash the assistant.
pub struct Dispatcher {
    tools: HashMap<String, ToolFn>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<F>(&mut self, name: impl Into<String>, fn_: F)
    where
        F: Fn(&HashMap<String, String>) -> ToolResult + Send + Sync + 'static,
    {
        self.tools.insert(name.into(), Box::new(fn_));
    }

    pub fn run(&self, intent: &Intent) -> ToolResult {
        match self.tools.get(&intent.name) {
            Some(fn_) => {
                // Catch panics so a bad tool never crashes the assistant
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fn_(&intent.slots))) {
                    Ok(result) => result,
                    Err(_) => ToolResult::failure("Sorry, something went wrong with that."),
                }
            }
            None => ToolResult::failure("I can't do that yet."),
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
