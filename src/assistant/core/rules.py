from assistant.core.intents import RuleEngine


def build_default_engine() -> RuleEngine:
    """The M1 rules. Order = priority: specific first, general later."""
    e = RuleEngine()

    e.add("get_time",
        r"\bwhat(?:'s| is)?\s+the\s+time\b|\bwhat\s+time\s+is\s+it\b"
        r"|\btell me the time\b|\bcurrent time\b")

    e.add("get_date",
        r"\bwhat(?:'s| is)?\s+(?:the\s+|today's\s+)?date\b"
        r"|\bwhat\s+day\s+is\s+(?:it|today)\b|\btoday's date\b")

    e.add("set_timer", r"\btimer\s+for\s+(?P<duration>.+)")
    e.add("set_timer", r"\b(?:set|start)\s+(?:a|an)\s+(?P<duration>.+?)\s+timer\b")
    e.add("set_timer", r"\b(?:set|start)\s+(?:a|an)\s+timer\b")

    e.add("set_volume", r"\bvolume\s+(?:to|at)\s+(?P<level>\d{1,3})\b")
    e.add("set_volume", r"\b(?:set|change) (?:the )?volume\b")
    e.add("volume_up",
            r"\bvolume up\b|\bturn (?:it |the volume )?up\b|\b(?:increase|raise) (?:the )?volume\b|\blouder\b")
    e.add("volume_down",
            r"\bvolume down\b|\bturn (?:it |the volume )?down\b"
            r"|\b(?:decrease|lower|reduce) (?:the )?volume\b|\bquieter\b|\bsofter\b")

    e.add("calculate",
              r"\b(?:what(?:'s| is)|calculate|compute|how much is)\s+(?P<expression>.*\d.*)")

    e.add("tell_joke", r"\bjokes?\b|\bmake me laugh\b")
    return e
