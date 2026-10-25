from src.assistant.core.intents import RuleEngine

#
# cases = {
#     "Hey Assistant, set a timer for 5 minutes, please.": "set a timer for 5 minutes",
#     "What’s 2+2?": "what's 2+2",
#     "Wake me at 5:30 tomorrow.": "wake me at 5:30 tomorrow",
#     "Set a five-minute timer": "set a five minute timer",
#     "  What   time is it?? ": "what time is it",
#     "Volume to 2.5": "volume to 2.5",
#     "Hey Pural, set a alarm for 5:30am, please.": "set a alarm for 5:30am",
#     "DO a perticular task for 10mints and leave the rest": "do a perticular task for 10mints and leave the rest",
#     "I got a date at 6:30 p.m.": "i got a date at 6:30 p m"
# }
# for raw, expected in cases.items():
#     got = normalize(raw)
#     print("OK " if got == expected else "BAD", repr(raw), "->", repr(got))

engine = RuleEngine()
engine.add("get_time", r"\bwhat(?:'s| is)? the time\b|\bwhat time is it\b")
engine.add("set_timer", r"\bset (?:a )?timer for (?P<duration>.+)")


tests = [
    "Hey Assistant, what time is it?",
    "Set a timer for 5 minutes.",
    "What's the time",
    "Set a five-minute timer",
    "Sing me a song",
    "",
]
for t in tests:
    print(repr(t), "->", engine.parse(t))
