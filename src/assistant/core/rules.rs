use crate::assistant::core::intents::RuleEngine; // adjust path if needed

/// The M1 rules. Order = priority: specific first, general later.
pub fn build_default_engine() -> RuleEngine {
    let mut e = RuleEngine::new();

    e.add(
        "get_time",
        r"\bwhat(?:'s| is)?\s+the\s+time\b|\bwhat\s+time\s+is\s+it\b|\btell me the time\b|\bcurrent time\b",
        1.0,
    )
    .expect("invalid regex");

    e.add(
        "get_date",
        r"\bwhat(?:'s| is)?\s+(?:the\s+|today's\s+)?date\b|\bwhat\s+day\s+is\s+(?:it|today)\b|\btoday's date\b",
        1.0,
    )
    .expect("invalid regex");

    e.add("set_timer", r"\btimer\s+for\s+(?P<duration>.+)", 1.0)
        .expect("invalid regex");

    e.add(
        "set_timer",
        r"\b(?:set|start)\s+(?:a|an)\s+(?P<duration>.+?)\s+timer\b",
        1.0,
    )
    .expect("invalid regex");

    e.add("set_timer", r"\b(?:set|start)\s+(?:a|an)\s+timer\b", 1.0)
        .expect("invalid regex");

    e.add(
        "set_volume",
        r"\bvolume\s+(?:to|at)\s+(?P<level>\d{1,3})\b",
        1.0,
    )
    .expect("invalid regex");

    e.add(
        "volume_up",
        r"\bvolume up\b|\bturn (?:it |the volume )?up\b|\b(?:increase|raise) (?:the )?volume\b|\blouder\b",
        1.0,
    )
    .expect("invalid regex");

    e.add(
        "volume_down",
        r"\bvolume down\b|\bturn (?:it |the volume )?down\b|\b(?:decrease|lower|reduce) (?:the )?volume\b|\bquieter\b|\bsofter\b",
        1.0,
    )
    .expect("invalid regex");

    e.add(
        "calculate",
        r"\b(?:what(?:'s| is)|calculate|compute|how much is)\s+(?P<expression>.*\d.*)",
        1.0,
    )
    .expect("invalid regex");

    e.add("tell_joke", r"\bjokes?\b|\bmake me laugh\b", 1.0)
        .expect("invalid regex");

    e
}
