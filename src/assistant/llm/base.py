from dataclasses import dataclass, field
from typing import Any, Literal, Protocol


class LLMError(Exception):
    """The backend couldn't produce a response (not working, timeout, bad model...)."""


@dataclass(frozen=True)
class Message:
    role: Literal["system", "user", "assistant"]
    content: str


@dataclass(frozen=True)
class ToolSpec:
    """A tool the model may call. `parameters` is a JSON Schema object."""
    name: str
    description: str
    parameters: dict[str, Any] = field(
        default_factory=lambda: {"type": "object", "properties": {}}
    )


@dataclass(frozen=True)
class ToolCall:
    name: str
    arguments: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class LLMResponse:
    text: str = ""
    tool_calls: tuple[ToolCall, ...] = ()


class LLMBackend(Protocol):
    def check(self) -> str | None:
        """None if ready to use, otherwise a human-readable problem description."""
        ...

    def chat(self,
        messages: list[Message],
        tools: list[ToolSpec] | None = None,
        json_schema: dict[str, Any] | None = None,
    ) -> LLMResponse:
        """One non-streaming request. Raises LLMError if the backend fails.
        With json_schema, the reply text is constrained to match that schema."""
        ...
