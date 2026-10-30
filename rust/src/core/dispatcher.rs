use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use crate::interfaces::{Intent, ToolResult};

/// Type alias for a tool function.
/// Takes the intent's slots and returns a `ToolResult`.
pub type ToolFn = Box<dyn Fn(&HashMap<String, String>) -> ToolResult + Send + Sync>;

/// Maps `Intent.name` to a tool function and never lets a tool crash the assistant.
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
        let Some(fn_) = self.tools.get(&intent.name) else {
            return ToolResult::err("I can't do that yet.");
        };

        // Catch panics so a buggy tool cannot take down the whole process.
        match panic::catch_unwind(AssertUnwindSafe(|| fn_(&intent.slots))) {
            Ok(result) => result,
            Err(_) => ToolResult::err("Sorry, something went wrong with that."),
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
