import re
from datetime import datetime, time, timedelta

from assistant.core.intents import normalize
from assistant.core.parsing import NUMBER_WORDS, parse_duration, words_to_digits

_DAYS = {"today": 0, "tomorrow": 1}
_MARKERS = {"in", "at", "today", "tomorrow"}
_TRAILING = {"by", "at", "on", "for", "around", "about"}
_DURATION_WORDS = (
    {"second", "seconds", "sec", "secs", "minute", "minutes", "min", "mins",
     "hour", "hours", "hr", "hrs", "and", "a", "an", "half"} | NUMBER_WORDS)
_NUMBER = re.compile(r"^\d+(?:\.\d+)?$")
_CLOCK = re.compile(
    r"^(?:(?P<noon>noon|midday)|(?P<midnight>midnight)"
    r"|(?P<h>\d{1,2})(?:[: ](?P<m>\d{2}))?\s*(?P<ap>a ?m|p ?m)?)$")
MAX_AHEAD = timedelta(days=365)


def _relative(tokens: list[str], now: datetime) -> datetime | None:
    """'20 minutes' -> now + 20 min. Every word must be part of a duration."""
    if not tokens or not all(t in _DURATION_WORDS or _NUMBER.match(t) for t in tokens):
        return None
    seconds = parse_duration(" ".join(tokens))
    if seconds is None or timedelta(seconds=seconds) > MAX_AHEAD:
        return None
    return now + timedelta(seconds=seconds)


def _clock(tokens: list[str], now: datetime) -> datetime | None:
    tokens = list(tokens)
    day = tokens.pop(0) if tokens and tokens[0] in _DAYS else None
    if tokens and tokens[0] == "at":
        tokens.pop(0)
    if day is None and tokens and tokens[-1] in _DAYS:
        day = tokens.pop()
    match = _CLOCK.match(" ".join(tokens))
    if not match:
        return None

    meridiem = (match["ap"] or "").replace(" ", "")
    if match["noon"]:
        hour, minute, exact = 12, 0, True
    elif match["midnight"]:
        hour, minute, exact = 0, 0, True
    else:
        hour, minute = int(match["h"]), int(match["m"] or 0)
        if minute > 59 or hour > 23 or (meridiem and not 1 <= hour <= 12):
            return None
        if meridiem:
            hour = hour % 12 + (12 if meridiem == "pm" else 0)
        exact = bool(meridiem) or hour == 0 or hour >= 13   # False: "5" could be AM or PM

    if day:
        if not exact and hour != 12 and hour <= 6:
            hour += 12            # "tomorrow at 5" means 5 PM; 7 to 11 stay AM; 12 is noon
        return datetime.combine(now.date() + timedelta(days=_DAYS[day]), time(hour, minute))

    options = [hour] if exact else [hour % 12, hour % 12 + 12]
    for offset in (0, 1):                                    # today, then tomorrow
        date = now.date() + timedelta(days=offset)
        candidates = [datetime.combine(date, time(h, minute)) for h in options]
        upcoming = [c for c in candidates if c > now]
        if upcoming:
            return min(upcoming)
    return None


def parse_when(text: str, now: datetime | None = None) -> datetime | None:
    """'in 20 minutes', 'at 5:30 pm', 'tomorrow at nine' -> datetime. None if not a time.
    The whole text must be a time phrase. The result is in the past only when the
    user said 'today' with a time that has already gone by."""
    now = now or datetime.now()
    tokens = words_to_digits(normalize(text)).split()
    if not tokens:
        return None
    if tokens[0] == "in":
        return _relative(tokens[1:], now)
    return _clock(tokens, now)


def split_reminder(text: str, now: datetime | None = None) -> tuple[str, str]:
    """'call mom tomorrow at 9' -> ('call mom', 'tomorrow at 9'). Missing parts are ''."""
    now = now or datetime.now()
    tokens = normalize(text).split()
    # Time first: "in twenty minutes to check the oven"
    if tokens and tokens[0] in _MARKERS:
        for j in range(1, len(tokens)):
            if tokens[j] == "to" and parse_when(" ".join(tokens[:j]), now):
                return " ".join(tokens[j + 1:]), " ".join(tokens[:j])
    # Time last: "check the oven in twenty minutes". The earliest valid split wins.
    for i, token in enumerate(tokens):
        if token in _MARKERS and parse_when(" ".join(tokens[i:]), now):
            task = list(tokens[:i])
            while task and task[-1] in _TRAILING:
                task.pop()
            return " ".join(task), " ".join(tokens[i:])
    return " ".join(tokens), ""
