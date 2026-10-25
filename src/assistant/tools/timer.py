import threading
from collections.abc import Callable

from assistant.core.parsing import parse_duration
from assistant.interfaces import ToolResult

MAX_SECONDS = 24 * 3600


def format_duration(seconds: int) -> str:
    """90 -> '1 minute 30 seconds'"""
    parts = []
    for unit, size in (("hour", 3600), ("minute", 60), ("second", 1)):
        count, seconds = divmod(seconds, size)
        if count:
            parts.append(f"{count} {unit}{'s' if count != 1 else ''}")
    return " ".join(parts)


class TimerManager:
    """Owns running timers. `on_fire` is how a finished timer reaches the user."""

    def __init__(self, on_fire: Callable[[str], None]) -> None:
        self._on_fire = on_fire
        self._timers: set[threading.Timer] = set()
        self._lock = threading.Lock()

    def set_timer(self, slots: dict[str,str]) -> ToolResult:
        raw = slots.get("duration")
        if raw is None:
            return ToolResult(False, "For how long?")
        seconds = parse_duration(raw)
        if seconds is None:
            return ToolResult(False, "Sorry, I didn't catch how long.")
        if seconds > MAX_SECONDS:
            return ToolResult(False, "That's longer that I can time. The limit is 24 hours.")

        label = format_duration(seconds)
        timer = threading.Timer(seconds, self._fire, args=(label,))
        timer.daemon = True
        with self._lock:
            self._timers.add(timer)
        timer.start()
        return ToolResult(True, f"Timer set for {label}.", {"seconds": seconds})

    def _fire(self, label: str) -> None:
        current = threading.current_thread()
        with self._lock:
            self._timers.discard(current)
        self._on_fire(f"Your timer for {label} is done.")

    def cancel_all(self) -> None:
        with self._lock:
            for t in self._timers:
                t.cancel()
            self._timers.clear()
