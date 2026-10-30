import queue


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
