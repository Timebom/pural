from dataclasses import dataclass

from assistant.core.dispatcher import Dispatcher
from assistant.core.intents import normalize
from assistant.interfaces import FollowUp, Intent, IntentEngine

CANCEL_PHRASES = {"cancel", "never mind", "nevermind", "forget it", "stop"}
MAX_RETRIES = 2 # how many failed answers before we give up


@dataclass(frozen=True)
class Reply:
    text: str
    awaiting_answer: bool = False # True: caller should listen again, no wake word


class Conversation:
    """Remembers whether the assistant is waiting for an answer."""

    def __init__(self, engine: IntentEngine, dispatcher: Dispatcher) -> None:
        self._engine = engine
        self._dispatcher = dispatcher
        self._pending: FollowUp | None = None
        self._retries = 0

    def cancel(self) -> None:
        self._pending = None
        self._retries = 0

    def respond(self, text: str) -> Reply:
        if self._pending is not None:
            cleaned = normalize(text)
            if cleaned in CANCEL_PHRASES:
                self.cancel()
                return Reply("Okay, never mind.")
            fresh = self._engine.parse(text)
            if fresh is None: # not a command: it's the answer
                p = self._pending
                filled = Intent(p.intent, {**p.slots, p.slot: cleaned})
                return self._run(filled, is_retry=True)
            self.cancel() # new command: drop the old question
            return self._run(fresh)

        intent = self._engine.parse(text)
        if intent is None:
            return Reply("Sorry, I didn't catch that.")
        return self._run(intent)

    def _run(self, intent: Intent, is_retry: bool = False) -> Reply:
        result = self._dispatcher.run(intent)
        if result.follow_up is None:
            self.cancel()
            return Reply(result.message)
        self._retries = self._retries + 1 if is_retry else 0
        if self._retries >= MAX_RETRIES:
            self.cancel()
            return Reply("Let's try that again later.")
        self._pending = result.follow_up
        return Reply(result.message, awaiting_answer=True)
