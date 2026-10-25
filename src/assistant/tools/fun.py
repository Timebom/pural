import random

from assistant.interfaces import ToolResult

_JOKES = [
    "Why do programmers prefer dark mode? Because light attracts bugs.",
    "I would tell you a UDP joke, but you might not get it.",
    "There are only 10 kinds of people: those who understand binary and those who don't.",
    "Why did the developer go broke? Because he used up all his cache.",
    "A SQL query walks into a bar, walks up to two tables and asks: can I join you?",
]
_last: str | None = None


def tell_joke(slots: dict[str, str]) -> ToolResult:
    global _last
    choices = [j for j in _JOKES if j != _last]
    _last = random.choice(choices)
    return ToolResult(True, _last)
