from assistant.interfaces import Intent, IntentEngine


class ChainEngine:
    """Tries each engine in order; the first one that understands wins."""

    def __init__(self, *engines: IntentEngine) -> None:
        self._engines = engines

    def parse(self, text: str) -> Intent | None:
        for engine in self._engines:
            intent = engine.parse(text)
            if intent is not None:
                return intent
        return None
