use crate::core::intents::RuleEngine; // adjust path as needed

/// The M1 rules. Order = priority: specific first, general later.
pub fn build_default_engine() -> RuleEngine {
    let mut e = RuleEngine::new();

    // Web search: explicit commands only.
    e.add(
        "web_search",
        r"\b(?:search|look up|google)\s+(?:(?:the\s+)?(?:web|internet|online)\s+)?(?:for\s+)?(?P<query>.+)",
        1.0,
    );
    e.add("web_search", r"\b(?:search|look up|google)\b", 1.0); // no query -> asks for one

    // Reminders.
    e.add(
        "delete_reminder",
        r"\b(?:delete|cancel|remove|clear)\s+(?P<task>all|every)\b.*\breminders?\b",
        1.0,
    );
    e.add(
        "delete_reminder",
        r"\b(?:delete|cancel|remove)\s+(?:my\s+|the\s+)?reminder\s+(?:about\s+|to\s+|for\s+)?(?P<task>.+)",
        1.0,
    );
    e.add(
        "delete_reminder",
        r"\b(?:delete|cancel|remove)\s+(?:my\s+|the\s+)?(?P<task>.+?)\s+reminders?\b",
        1.0,
    );
    e.add(
        "delete_reminder",
        r"\b(?:delete|cancel|remove|clear)\b.*\breminders?\b",
        1.0,
    );
    e.add("list_reminders", r"\breminders\b|\bnext reminder\b", 1.0);
    e.add(
        "set_reminder",
        r"\b(?:set|add|create|make)\s+(?:a\s+|an\s+)?reminder\s+(?:to\s+|about\s+|for\s+|that\s+)?(?P<text>.+)",
        1.0,
    );
    e.add(
        "set_reminder",
        r"\bremind me\s+(?:to\s+|about\s+|that\s+)?(?P<text>.+)",
        1.0,
    );
    e.add(
        "set_reminder",
        r"\b(?:set|add|create|make)\s+(?:a\s+|an\s+)?reminder\b|\bremind me\b",
        1.0,
    );

    // Weather: with a place, then with only a day, then bare.
    e.add(
        "get_weather",
        r"\b(?:weather|forecast)\b.*?\b(?:in|at|for)\s+(?P<location>.+)",
        1.0,
    );
    e.add(
        "get_weather",
        r"\b(?:weather|forecast)\b.*?\b(?P<day>tomorrow)\b",
        1.0,
    );
    e.add(
        "get_weather",
        r"\b(?P<day>tomorrow)'s\s+(?:weather|forecast)\b",
        1.0,
    );
    e.add("get_weather", r"\b(?:weather|forecast)\b", 1.0);

    // Get Time or Date
    e.add(
        "get_time",
        r"\bwhat(?:'s| is)?\s+the\s+time\b|\bwhat\s+time\s+is\s+it\b|\btell me the time\b|\bcurrent time\b",
        1.0,
    );
    e.add(
        "get_date",
        r"\bwhat(?:'s| is)?\s+(?:the\s+|today's\s+)?date\b|\bwhat\s+day\s+is\s+(?:it|today)\b|\btoday's date\b",
        1.0,
    );

    // Set Timer
    e.add("set_timer", r"\btimer\s+for\s+(?P<duration>.+)", 1.0);
    e.add(
        "set_timer",
        r"\b(?:set|start)\s+(?:a|an)\s+(?P<duration>.+?)\s+timer\b",
        1.0,
    );
    e.add(
        "set_timer",
        r"\b(?:set|start)\s+(?:(?:a|an)\s+)?timer\b",
        1.0,
    );

    // Volume Control
    e.add("set_volume", r"\bvolume\s+(?:to|at)\s+(?P<level>.+)", 1.0);
    e.add("set_volume", r"\b(?:set|change) (?:the )?volume\b", 1.0);
    e.add(
        "volume_up",
        r"\bvolume up\b|\bturn (?:it |the volume )?up\b|\b(?:increase|raise) (?:the )?volume\b|\blouder\b",
        1.0,
    );
    e.add(
        "volume_down",
        r"\bvolume down\b|\bturn (?:it |the volume )?down\b|\b(?:decrease|lower|reduce) (?:the )?volume\b|\bquieter\b|\bsofter\b",
        1.0,
    );

    // Math Solve
    e.add(
        "calculate",
        r"\b(?:what(?:'s| is)|calculate|compute|how much is)\s+(?P<expression>.*\d.*)",
        1.0,
    );

    // Extras
    e.add("tell_joke", r"\bjokes?\b|\bmake me laugh\b", 1.0);

    e
}
