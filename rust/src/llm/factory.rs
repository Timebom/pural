use crate::config::LLMSettings;
use super::base::{LLMBackend, LLMError};
use super::ollama_backend::OllamaBackend;

/// Factory that creates the concrete LLM backend based on configuration.
pub fn create_backend(cfg: &LLMSettings) -> Result<Box<dyn LLMBackend>, LLMError> {
    match cfg.backend.as_str() {
        "ollama" => Ok(Box::new(OllamaBackend::new(
            cfg.model.clone(),
            cfg.host.clone(),
            cfg.temperature,
            cfg.max_tokens,
            cfg.keep_alive.clone(),
            cfg.timeout,
        ))),
        other => Err(LLMError(format!("Unknown LLM backend: {:?}", other))),
    }
}
