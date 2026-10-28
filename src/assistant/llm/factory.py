from assistant.config import LLMSettings
from assistant.llm.base import LLMBackend


def create_backend(cfg: LLMSettings) -> LLMBackend:
    if cfg.backend == "ollama":
        from assistant.llm.ollama_backend import OllamaBackend  # lazy: only if needed.
        return OllamaBackend(
            cfg.model,
            cfg.host,
            cfg.temperature,
            cfg.max_tokens,
            cfg.keep_alive,
            cfg.timeout
        )
    raise ValueError(f"Unknown LLM backend: {cfg.backend!r}")
