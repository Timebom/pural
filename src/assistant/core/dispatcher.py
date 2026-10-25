import logging
from collections.abc import Callable

from assistant.interfaces import Intent, ToolResult

log = logging.getLogger(__name__)

ToolFn = Callable[[dict[str,str]], ToolResult]


class Dispatcher:
    """Maps Intent.name to a tool function and never lets a tool crash the assistant."""

    def __init__(self) -> None:
        self._tools: dict[str, ToolFn] = {}

    def register(self, name: str, fn: ToolFn) -> None:
        self._tools[name] = fn

    def run(self, intent: Intent) -> ToolResult:
        fn = self._tools.get(intent.name)
        if fn is None:
            return ToolResult(False, "I can't do that yet.")
        try:
            return fn(intent.slots)
        except Exception:
            log.exception("Tool %s failed", intent.name)
            return ToolResult(False, "Sorry, something went wrong with that.")
