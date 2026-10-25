from datetime import datetime

from assistant.interfaces import ToolResult


def get_time(slots: dict[str, str]) -> ToolResult:
    now = datetime.now()
    hour12 = now.hour % 12 or 12
    suffix = "AM" if now.hour < 12 else "PM"
    spoken = f"{hour12}:{now.minute:02d} {suffix}"
    return ToolResult(True, f"It's {spoken}.", {"iso": now.isoformat()})


def get_date(slots: dict[str, str]) -> ToolResult:
    now = datetime.now()
    spoken = f"{now:%A}, {now:%B} {now.day}, {now.year}"
    return ToolResult(True, f"Today is {spoken}.", {"iso": now.date().isoformat()})
