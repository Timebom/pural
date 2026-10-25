import threading

from assistant.audio import play
from assistant.interfaces import TTS


class Speaker:
    """Speak text one utterance at a time, safe to call from any thread."""

    def __init__(self, tts: TTS) -> None:
        self._tts = tts
        self._lock = threading.Lock()

    def say(self, text: str) -> None:
        with self._lock: # a timer can't talk over a reply
            samples, rate = self._tts.synthesize(text)
            play(samples, rate)
