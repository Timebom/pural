import httpx2
from ollama import Client, ResponseError

from assistant.llm.base import LLMError, LLMResponse, Message, ToolCall, ToolSpec

_UNREACHABLE = (OSError, httpx2.HTTPError) # server down, DNS, timeout, refused...


def _to_ollama_tool(spec: ToolSpec) -> dict:
    return {
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.parameters
        }
    }


class OllamaBackend:
    """Satisfies LLMBackend by talking to a local Ollama server."""

    def __init__(
        self,
        model: str = "llama3.2",
        host: str = "http://localhost:11434",
        temperature: float = 0.2,
        max_tokens: int = 200,
        keep_alive: str = "30m",
        timeout: float = 60.0,
    ) -> None:
        self._model = model
        self._host = host
        self._options = {"temperature": temperature, "num_predict": max_tokens}
        self._keep_alive = keep_alive
        self._client = Client(host=host, timeout=timeout)

    def check(self) -> str | None:
        try:
            self._client.show(self._model)
        except ResponseError as e:
            if getattr(e, "status_code", None) == 404:
                return f"model '{self._model}' not found. Run: ollama pull {self._model}"
            return f"Ollama error: {e}"
        except _UNREACHABLE:
            return f"can't reach Ollama at {self._host}. Is it running?"
        return None

    def chat(
        self,
        messages: list[Message],
        tools: list[ToolSpec] | None = None,
        json_schema: dict | None = None,
    ) -> LLMResponse:
        try:
            response = self._client.chat(
                model=self._model,
                messages=[{"role": m.role, "content": m.content} for m in messages],
                tools=[_to_ollama_tool(t) for t in tools] if tools else None,
                format=json_schema,
                stream=False,
                options=self._options,
                keep_alive=self._keep_alive,
            )
        except (ResponseError, *_UNREACHABLE) as e:
            raise LLMError(str(e)) from e

        msg = response.message
        calls = tuple(ToolCall(c.function.name, dict(c.function.arguments))
            for c in (msg.tool_calls or []))
        return LLMResponse((msg.content or  "").strip(), calls)
