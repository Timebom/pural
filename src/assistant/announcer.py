import logging
import queue
import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager

log = logging.getLogger(__name__)


class Announcer:
    """Thread-safe mailbox for messages from background threads (timers, reminders)."""

    def __init__(self) -> None:
        self._queue: queue.SimpleQueue[str] = queue.SimpleQueue()

    def post(self, message: str) -> None:
        self._queue.put(message)

    def pending(self) -> bool:
        return not self._queue.empty()

    def drain(self) -> list[str]:
        messages = []
        while True:
            try:
                messages.append(self._queue.get_nowait())
            except queue.Empty:
                return messages


class AudioGate:
    """Keeps background messages from playing while the mic is recording.

    Background threads call deliver(). If the mic is recording, the message is queued
    for the main loop to speak later. Otherwise it is spoken right away, and recording()
    waits for that speech to finish before the mic opens. One lock makes both true."""

    def __init__(self, speak: Callable[[str], None], announcer: Announcer) -> None:
        self._speak = speak
        self._announcer = announcer
        self._lock = threading.Lock()
        self._recording = False

    def deliver(self, message: str) -> None:
        with self._lock:
            if self._recording:
                self._announcer.post(message)
                return
            try:
                self._speak(message)
            except Exception: # a failing speaker must not kill a timer thread
                log.exception("could not speak announcement")

    @contextmanager
    def recording(self) -> Iterator[None]:
        with self._lock: # waits here if a message is being spoken
            self._recording = True
        try:
            yield
        finally:
            with self._lock:
                self._recording = False
