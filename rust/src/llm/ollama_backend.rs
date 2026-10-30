use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::time::Duration;

use super::base::{
    LLMBackend, LLMError, LLMResponse, Message, Role, ToolCall, ToolSpec,
};

fn to_ollama_tool(spec: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.parameters
        }
    })
}

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a Value>,
    stream: bool,
    options: &'a Map<String, Value>,
    keep_alive: &'a str,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: HashMap<String, Value>,
}

/// Satisfies `LLMBackend` by talking to a local Ollama server.
pub struct OllamaBackend {
    model: String,
    host: String,
    options: Map<String, Value>,
    keep_alive: String,
    client: Client,
}

impl OllamaBackend {
    pub fn new(
        model: impl Into<String>,
        host: impl Into<String>,
        temperature: f32,
        max_tokens: u32,
        keep_alive: impl Into<String>,
        timeout_secs: u64,
    ) -> Self {
        let mut options = Map::new();
        options.insert("temperature".into(), json!(temperature));
        options.insert("num_predict".into(), json!(max_tokens));

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs as u64))
            .build()
            .expect("failed to build reqwest client");

        Self {
            model: model.into(),
            host: host.into(),
            options,
            keep_alive: keep_alive.into(),
            client,
        }
    }
}

impl LLMBackend for OllamaBackend {
    fn check(&self) -> Option<String> {
        let url = format!("{}/api/show", self.host.trim_end_matches('/'));

        match self.client.post(&url).json(&json!({ "name": self.model })).send() {
            Ok(resp) => {
                if resp.status() == StatusCode::NOT_FOUND {
                    Some(format!(
                        "model '{}' not found. Run: ollama pull {}",
                        self.model, self.model
                    ))
                } else if !resp.status().is_success() {
                    Some(format!("Ollama error: {}", resp.status()))
                } else {
                    None
                }
            }
            Err(e) => {
                // Covers connection refused, DNS, timeout, etc.
                Some(format!("can't reach Ollama at {}. Is it running? ({e})", self.host))
            }
        }
    }

    fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[ToolSpec]>,
        json_schema: Option<&Value>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!("{}/api/chat", self.host.trim_end_matches('/'));

        let ollama_messages: Vec<OllamaMessage> = messages
            .iter()
            .map(|m| OllamaMessage {
                role: role_to_str(&m.role),
                content: &m.content,
            })
            .collect();

        let tools_json = tools.map(|ts| ts.iter().map(to_ollama_tool).collect());

        let body = OllamaChatRequest {
            model: &self.model,
            messages: ollama_messages,
            tools: tools_json,
            format: json_schema,
            stream: false,
            options: &self.options,
            keep_alive: &self.keep_alive,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| LLMError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(LLMError(format!("Ollama error {status}: {text}")));
        }

        let parsed: OllamaChatResponse = resp
            .json()
            .map_err(|e| LLMError(format!("failed to parse Ollama response: {e}")))?;

        let text = parsed
            .message
            .content
            .unwrap_or_default()
            .trim()
            .to_string();

        let tool_calls = parsed
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|c| ToolCall {
                name: c.function.name,
                arguments: c.function.arguments,
            })
            .collect();

        Ok(LLMResponse {
            text,
            tool_calls,
        })
    }
}
