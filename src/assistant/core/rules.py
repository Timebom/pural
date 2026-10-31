from assistant.core.intents import RuleEngine


def build_default_engine() -> RuleEngine:
    """The M1 rules. Order = priority: specific first, general later."""
    e = RuleEngine()

    # Reminders.
    e.add("delete_reminder", r"\b(?:delete|cancel|remove|clear)\s+(?P<task>all|every)\b.*\breminders?\b")
    e.add("delete_reminder", r"\b(?:delete|cancel|remove)\s+(?:my\s+|the\s+)?reminder\s+(?:about\s+|to\s+|for\s+)?(?P<task>.+)")
    e.add("delete_reminder", r"\b(?:delete|cancel|remove)\s+(?:my\s+|the\s+)?(?P<task>.+?)\s+reminders?\b")
    e.add("delete_reminder", r"\b(?:delete|cancel|remove|clear)\b.*\breminders?\b")
    e.add("list_reminders", r"\breminders\b|\bnext reminder\b")
    e.add("set_reminder", r"\b(?:set|add|create|make)\s+(?:a\s+|an\s+)?reminder\s+(?:to\s+|about\s+|for\s+|that\s+)?(?P<text>.+)")
    e.add("set_reminder", r"\bremind me\s+(?:to\s+|about\s+|that\s+)?(?P<text>.+)")
    e.add("set_reminder", r"\b(?:set|add|create|make)\s+(?:a\s+|an\s+)?reminder\b|\bremind me\b")

    # System status: battery, cpu, memory, disk, or a general check.
    e.add("system_status", r"\b(?P<what>battery)\b")
    e.add("system_status", r"\b(?P<what>cpu|processor)\b")
    e.add("system_status", r"\b(?P<what>memory|ram|disk|storage)\s+(?:usage|space)\b")
    e.add("system_status", r"\bhow much\s+(?:free\s+)?(?P<what>memory|ram|disk|storage)\b")
    e.add("system_status", r"\b(?:my|computer'?s?|system)\s+(?P<what>memory|ram|disk|storage)\b")
    e.add("system_status", r"\b(?:system|computer|laptop)\s+(?:status|stats|health)\b|\bhow(?:'s| is) (?:my|the) (?:computer|laptop|mac|system|machine)\b")

    # Web search: explicit commands only.
    e.add("web_search",
        r"\b(?:search|look up|google)\s+(?:(?:the\s+)?(?:web|internet|online)\s+)?(?:for\s+)?(?P<query>.+)")
    e.add("web_search", r"\b(?:search|look up|google)\b")        # no query -> asks for one

    # Alarms. Order matters: cancel, then list, then set.
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?P<which>all)\b.*\balarms?\b")
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?:my\s+|the\s+)?alarms?\s+(?:for|at)\s+(?P<which>.+)")
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?:my\s+|the\s+)?(?:(?P<which>.+?)\s+)?alarms?\b")
    e.add("list_alarms", r"\balarms\b|\balarm\s+(?:is\s+)?set\b")
    e.add("set_alarm", r"\b(?:set|add|create|make)\s+(?:an?\s+)?alarm\s+(?:for\s+|at\s+)?(?P<when>.+)")
    e.add("set_alarm", r"\b(?:set|add|create|make)\s+(?:an?\s+)?alarm\b")
    e.add("set_alarm", r"\balarm\s+(?:for|at)\s+(?P<when>.+)")
    e.add("set_alarm", r"\bwake me(?: up)?\s+(?P<when>(?!up\b).+)")
    e.add("set_alarm", r"\bwake me\b")

    # Weather: with a place, then with only a day, then bare.
    e.add("get_weather", r"\b(?:weather|forecast)\b.*?\b(?:in|at|for)\s+(?P<location>.+)")
    e.add("get_weather", r"\b(?:weather|forecast)\b.*?\b(?P<day>tomorrow)\b")
    e.add("get_weather", r"\b(?P<day>tomorrow)'s\s+(?:weather|forecast)\b")
    e.add("get_weather", r"\b(?:weather|forecast)\b")

    # Get Time or Date
    e.add("get_time",
        r"\bwhat(?:'s| is)?\s+the\s+time\b|\bwhat\s+time\s+is\s+it\b"
        r"|\btell me the time\b|\bcurrent time\b")
    e.add("get_date",
        r"\bwhat(?:'s| is)?\s+(?:the\s+|today's\s+)?date\b"
        r"|\bwhat\s+day\s+is\s+(?:it|today)\b|\btoday's date\b")

    # Set Timer
    e.add("set_timer", r"\btimer\s+for\s+(?P<duration>.+)")
    e.add("set_timer", r"\b(?:set|start)\s+(?:a|an)\s+(?P<duration>.+?)\s+timer\b")
    e.add("set_timer", r"\b(?:set|start)\s+(?:(?:a|an)\s+)?timer\b")

    # Volume Control
    e.add("set_volume", r"\bvolume\s+(?:to|at)\s+(?P<level>.+)")
    e.add("set_volume", r"\b(?:set|change) (?:the )?volume\b")
    e.add("volume_up",
            r"\bvolume up\b|\bturn (?:it |the volume )?up\b|\b(?:increase|raise) (?:the )?volume\b|\blouder\b")
    e.add("volume_down",
            r"\bvolume down\b|\bturn (?:it |the volume )?down\b"
            r"|\b(?:decrease|lower|reduce) (?:the )?volume\b|\bquieter\b|\bsofter\b")

    # Math Solve
    e.add("calculate",
              r"\b(?:what(?:'s| is)|calculate|compute|how much is)\s+(?P<expression>.*\d.*)")

    # Extras
    e.add("tell_joke", r"\bjokes?\b|\bmake me laugh\b")
    return e
