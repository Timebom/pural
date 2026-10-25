from src.assistant.core.parsing import parse_duration
from src.assistant.core.rules import build_default_engine

durations = {
    "5 minutes": 300, "five minutes": 300, "10 minute": 600, "an hour": 3600,
    "half an hour": 1800, "two and a half hours": 9000, "1 hour 30 minutes": 5400,
    "twenty five seconds": 25, "2.5 hours": 9000, "five": None, "soon": None,
}
durations["a half hour"] = 1800
durations["one and a half minutes"] = 90
for text, want in durations.items():
    got = parse_duration(text)
    print("OK " if got == want else "BAD", repr(text), "->", got)

engine = build_default_engine()
for t in [
    "What time is it?", "What's the date today?", "Set a timer for five minutes",
    "Set a 10 minute timer", "Set a timer", "Set the volume to 40", "Turn it up",
    "Make it quieter", "What's 12 times 3?", "What's 2+2", "Tell me a joke",
    "Sing me a song",
]:
    print(repr(t), "->", engine.parse(t))
