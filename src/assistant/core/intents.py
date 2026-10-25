import re
from dataclasses import dataclass

from assistant.interfaces import Intent

_NUM_PUNCT = re.compile(r"(?<!\d)[.:]|[.:](?!\d)")
_HYPHEN_IN_WORD = re.compile(r"(?<=[a-z])-(?=[a-z])")
_UNWANTED = re.compile(r"[^a-z0-9\s.:+\-*/%^']")
_WAKE_PREFIX = re.compile(r"^(?:(?:hey|ok|okay)\s+)?(?:assistant|pural)\b\s*")
_POLITE = re.compile(r"\b(?:please|kindly)\b")
_SPACES = re.compile(r"\s+")

def normalize(text: str) -> str:
    """Lowercase, strip punctuation and filler so rules see clean text."""
    text = text.lower().replace("’", "'")
    text = _NUM_PUNCT.sub(" ", text)
    text = _HYPHEN_IN_WORD.sub(" ", text)
    text = _UNWANTED.sub(" ", text)
    text = _SPACES.sub(" ", text).strip()
    text = _WAKE_PREFIX.sub("", text)
    text = _POLITE.sub(" ", text)
    return _SPACES.sub(" ", text).strip()


@dataclass(frozen=True)
class Rule:
    name: str
    pattern: re.Pattern
    confidence: float = 1.0


class RuleEngine:
    """Satisfies the IntentEngine protocol: parse(text) -> Intent | None."""

    def __init__(self) -> None:
        self._rules: list[Rule] = []

    def add(self, name: str, pattern: str, confidence: float = 1.0) -> None:
        """Register a rule. Rules are tried in the order they are added."""
        self._rules.append(Rule(name, re.compile(pattern), confidence))

    def parse(self, text: str) -> Intent | None:
        cleaned = normalize(text)
        if not cleaned:
            return None
        for rule in self._rules:
            match = rule.pattern.search(cleaned)
            if match:
                slots = {k: v for k, v in match.groupdict().items() if v is not None}
                return Intent(rule.name, slots, rule.confidence)
        return None
