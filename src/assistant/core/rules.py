from assistant.core.intents import RuleEngine

_MACHINE = r"(?:laptop|computer|mac|macbook|pc|machine|system)"
_THE_MACHINE = rf"(?:the\s+|my\s+|this\s+)?{_MACHINE}"
_RESOURCE = r"(?:memory|ram|disk|storage|battery|cpu|processor)"
# Status questions only count when they are not "how do I ..." questions.
_STATUS = r"^(?!.*\bhow\s+(?:do|can|could|would|should|to)\b).*?"
_POLITE = r"^(?:(?:can|could|would|will)\s+you\s+)?"


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
    # System status. These fire only when the words clearly mean THIS computer.
    # Looser phrasings are left to the LLM, so "how much storage does an iPhone have" stays a question.
    e.add("system_status",
        _STATUS + r"\bmy\s+(?:(?:free|available|disk|hard drive|laptop|computer|mac)(?:'s)?\s+)*"
        r"(?P<what>ram|disk|storage|battery|cpu|processor"
        r"|memory(?=\s+(?:usage|use|status|free|left|space|is\s+(?:full|free|low|used))))\b") # my battery, my disk space
    e.add("system_status",
          _STATUS + rf"\b(?P<what>{_RESOURCE})\b.*\b(?:on|of|in|does|has|for)\s+(?:this|my)\s+{_MACHINE}\b") # memory on this computer
    e.add("system_status",
          _STATUS + rf"\b(?:this|my)\s+{_MACHINE}(?:'s)?\s+(?:(?:free|available)\s+)?(?P<what>{_RESOURCE})\b") # my computer's memory
    e.add("system_status",
          _STATUS + r"\bhow much\s+(?:(?:free|available|disk)\s+)*(?P<what>memory|ram|disk|storage|battery)"
          r"(?:\s+space)?\s+(?:do i have|do we have|have i got|am i using|is\s+(?:free|left|available|used|remaining))\b") # how much storage do I have
    e.add("system_status",
          _STATUS + r"\b(?P<what>battery)\s+(?:level|percentage|percent|charge|status)\b"
          r"(?!\s+(?:of|for|in|on)\b)") # battery level
    e.add("system_status",
          _STATUS + rf"(?:\b{_MACHINE}\s+(?:status|stats|health)\b"
          rf"|\bhow(?:'s| is)\s+(?:my|this)\s+{_MACHINE}\b)") # a general check

    # Web search: explicit commands only.
    e.add("web_search",
        r"\b(?:search|look up|google)\s+(?:(?:the\s+)?(?:web|internet|online)\s+)?(?:for\s+)?(?P<query>.+)")
    e.add("web_search", r"\b(?:search|look up|google)\b")        # no query -> asks for one

    # Alarms. Order matters: cancel, then list, then set.
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?P<which>all)\b.*\balarms?\b")
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?:my\s+|the\s+)?alarms?\s+(?:for|at)\s+(?P<which>.+)")
    e.add("cancel_alarm", r"\b(?:cancel|delete|remove|clear)\s+(?:my\s+|the\s+)?(?:(?P<which>.+?)\s+)?alarms?\b")
    e.add("list_alarms", r"\balarms\b|\balarm\s+(?:is\s+)?set\b")
    e.add("set_alarm", r"\b(?:set|add|create|make)\s+(?:an?\s+)?(?P<repeat>daily)\s+alarm\b"
        r"(?:\s+(?:for\s+|at\s+)?(?P<when>.+))?")
    e.add("set_alarm", r"\b(?:set|add|create|make)\s+(?:an?\s+)?alarm\s+(?:for\s+|at\s+)?(?P<when>.+)")
    e.add("set_alarm", r"\b(?:set|add|create|make)\s+(?:an?\s+)?alarm\b")
    e.add("set_alarm", r"\balarm\s+(?:for|at)\s+(?P<when>.+)")
    e.add("set_alarm", r"\bwake me(?: up)?\s+(?P<when>(?!up\b).+)")
    e.add("set_alarm", r"\bwake me\b")

    # Power. Deliberately narrow and whole-sentence: "shut down the server" must not match.
    # There are no rules for "go to sleep" (people say it to the assistant itself).
    e.add("shutdown", _POLITE + rf"(?:shut\s*down|power\s+(?:off|down))(?:\s+{_THE_MACHINE})?\s*$")
    e.add("shutdown", _POLITE + rf"(?:turn|switch)\s+off\s+{_THE_MACHINE}\s*$")
    e.add("shutdown", _POLITE + rf"(?:turn|switch)\s+{_THE_MACHINE}\s+off\s*$")
    e.add("suspend", _POLITE + rf"(?:suspend|hibernate)(?:\s+{_THE_MACHINE})?\s*$")
    e.add("suspend", _POLITE + rf"put\s+{_THE_MACHINE}\s+to\s+sleep\s*$")

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
        r"\b(?:what(?:'s| is)|calculate|compute|how much is)\s+"
        r"(?P<expression>(?=.*\d)(?=.*(?:\d\s*[-+*/^%x]\s*\d|\b(?:plus|minus|times|divided by"
        r"|multiplied by|over|to the power of|squared|percent)\b)).*)")

    # Extras
    e.add("tell_joke", r"\bjokes?\b|\bmake me laugh\b")
    return e
