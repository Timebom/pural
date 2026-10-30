use serde_json::{Map, Value};
use std::collections::HashMap;

/// The backend couldn't produce a response (not working, timeout, bad model...).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct LLMError(pub String);

impl From<&str> for LLMError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for LLMError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// A tool the model may call. `parameters` is a JSON Schema object.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: Value::Object(Map::from_iter([
                ("type".into(), Value::String("object".into())),
                ("properties".into(), Value::Object(Map::new())),
            ])),
        }
    }

    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub arguments: HashMap<String, Value>,
}

impl ToolCall {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: HashMap::new(),
        }
    }

    pub fn with_arguments(mut self, arguments: HashMap<String, Value>) -> Self {
        self.arguments = arguments;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LLMResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

impl LLMResponse {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tool_calls: Vec::new(),
        }
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }
}

/// Trait for LLM backends.
pub trait LLMBackend {
    /// `None` if ready to use, otherwise a human-readable problem description.
    fn check(&self) -> Option<String>;

    /// One non-streaming request.
    ///
    /// Raises `LLMError` if the backend fails.
    /// With `json_schema`, the reply text is constrained to match that schema.
    fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
        json_schema: Option<&Value>,
    ) -> Result<LLMResponse, LLMError>;
}
