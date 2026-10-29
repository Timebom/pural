import difflib
import json
import logging
import os
import threading
import time
import uuid
from collections.abc import Callable
from datetime import datetime
from pathlib import Path

from assistant.core.intents import normalize
from assistant.core.timeparse import parse_when, split_reminder
from assistant.interfaces import FollowUp, ToolResult

log = logging.getLogger(__name__)

_FIELDS = {"id", "task", "due", "status"}
_PRONOUNS = {"my": "your", "me": "you", "i": "you", "mine": "yours", "myself": "yourself",
             "i'm": "you're", "i'll": "you'll", "i've": "you've", "i'd": "you'd"}
_FILLER = {"the", "my", "a", "an", "that", "about", "to", "reminder", "reminders"}
_ALL = {"all", "every", "everything"}


def to_second_person(text: str) -> str:
    """'call my mom' -> 'call your mom'. The user said it about themselves."""
    return " ".join(_PRONOUNS.get(word, word) for word in text.split())


def describe_time(due: datetime, now: datetime) -> str:
    hour12 = due.hour % 12 or 12
    suffix = "AM" if due.hour < 12 else "PM"
    clock = f"{hour12} {suffix}" if due.minute == 0 else f"{hour12}:{due.minute:02d} {suffix}"
    days = (due.date() - now.date()).days
    if days == 0:
        return f"today at {clock}"
    if days == 1:
        return f"tomorrow at {clock}"
    if days == -1:
        return f"yesterday at {clock}"
    if 2 <= days < 7:
        return f"on {due:%A} at {clock}"
    return f"on {due:%B} {due.day} at {clock}"


def _matches(query: str, task: str) -> bool:
    """Every query word must (nearly) match a word in the task: 'dentest' finds 'dentist'."""
    task = task.lower()
    if query in task:
        return True
    task_words = task.split()
    return all(any(difflib.SequenceMatcher(None, w, t).ratio() >= 0.8 for t in task_words)
               for w in query.split())


class ReminderService:
    def __init__(self, path: str | Path, on_fire: Callable[[str], None],
                 poll_seconds: float = 1.0) -> None:
        self._path = Path(path)
        self._on_fire = on_fire
        self._poll = poll_seconds
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._items = self._load()

    def _load(self) -> list[dict]:
        try:
            data = json.loads(self._path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return []
        except (OSError, json.JSONDecodeError) as e:
            log.warning("reminders file unreadable (%s); starting empty", e)
            try:
                os.replace(self._path, self._path.with_suffix(".corrupt"))   # keep a copy
            except OSError:
                pass
            return []
        if not isinstance(data, list):
            return []
        recent = time.time() - 7 * 86400              # finished reminders are kept a week
        return [r for r in data if isinstance(r, dict) and _FIELDS <= r.keys()
                and (r["status"] == "pending" or r["due"] >= recent)]

    def _save(self) -> None:                           # caller must hold the lock
        self._path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self._path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self._items, indent=2), encoding="utf-8")
        os.replace(tmp, self._path)                    # atomic: never a half-written file

    def start(self) -> None:
        if self._thread is None:
            self._thread = threading.Thread(target=self._run, name="reminders", daemon=True)
            self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def _run(self) -> None:
        while not self._stop.wait(self._poll):
            try:
                self.fire_due()
            except Exception:                          # a background thread must not die silently
                log.exception("reminder check failed")

    def fire_due(self, now: float | None = None) -> int:
        """Speak every pending reminder whose time has come. Returns how many fired."""
        now = time.time() if now is None else now
        with self._lock:
            due = [r for r in self._items if r["status"] == "pending" and r["due"] <= now]
            for r in due:
                r["status"] = "done"                   # mark first: at most once, even if speaking fails
            if due:
                self._save()
        for r in due:                                  # speak outside the lock
            self._on_fire(self._fire_message(r, now))
        return len(due)

    @staticmethod
    def _fire_message(r: dict, now: float) -> str:
        task = to_second_person(r["task"])
        if now - r["due"] > 120:                       # came due while we weren't running
            when = describe_time(datetime.fromtimestamp(r["due"]), datetime.fromtimestamp(now))
            return f"You missed a reminder from {when}: {task}."
        return f"Reminder: {task}."

    def set_reminder(self, slots: dict[str, str]) -> ToolResult:
        task, when = slots.get("task", "").strip(), slots.get("when", "").strip()
        if slots.get("text", "").strip():
            task, when = split_reminder(slots["text"])
        if not task and not when:
            return ToolResult(False, "What should I remind you about?",
                              follow_up=FollowUp("set_reminder", "text"))
        if not task:
            return ToolResult(False, "Remind you to do what?",
                              follow_up=FollowUp("set_reminder", "task", {"when": when}))
        ask_when = FollowUp("set_reminder", "when", {"task": task})
        if not when:
            return ToolResult(False, "When should I remind you?", follow_up=ask_when)
        now = datetime.now()
        due = parse_when(when, now)
        if due is None:
            return ToolResult(False, "Sorry, I didn't catch when. When should I remind you?",
                              follow_up=ask_when)
        if due <= now:
            return ToolResult(False, "That time has already passed. When should I remind you?",
                              follow_up=ask_when)

        entry = {"id": uuid.uuid4().hex[:8], "task": task, "due": due.timestamp(),
                 "status": "pending", "created": time.time()}
        try:
            with self._lock:
                self._items.append(entry)
                self._save()
        except OSError as e:
            log.error("could not save reminder: %s", e)
            with self._lock:
                self._items.remove(entry)
            return ToolResult(False, "I couldn't save that reminder.")
        return ToolResult(True, f"Okay, I'll remind you to {to_second_person(task)} "
                                f"{describe_time(due, now)}.", {"id": entry["id"]})

    def list_reminders(self, slots: dict[str, str]) -> ToolResult:
        now = datetime.now()
        with self._lock:
            pending = sorted((r for r in self._items if r["status"] == "pending"),
                             key=lambda r: r["due"])
        if not pending:
            return ToolResult(True, "You have no reminders.")
        spoken = [f"{to_second_person(r['task'])} "
                  f"{describe_time(datetime.fromtimestamp(r['due']), now)}" for r in pending[:5]]
        intro = ("You have one reminder: " if len(pending) == 1
                 else f"You have {len(pending)} reminders: ")
        message = intro + ". ".join(spoken) + "."
        if len(pending) > 5:
            message += f" And {len(pending) - 5} more."
        return ToolResult(True, message, {"count": len(pending)})

    def delete_reminder(self, slots: dict[str, str]) -> ToolResult:
        words = [w for w in normalize(slots.get("task", "")).split() if w not in _FILLER]
        query = " ".join(words)
        if not query:
            return ToolResult(False, "Which reminder?",
                              follow_up=FollowUp("delete_reminder", "task"))
        with self._lock:
            pending = [r for r in self._items if r["status"] == "pending"]
            matches = pending if query in _ALL else [r for r in pending
                                                     if _matches(query, r["task"])]
            if not matches:
                return ToolResult(False, f"I couldn't find a reminder about {query}.")
            if len(matches) > 1 and query not in _ALL:
                names = ", and ".join(to_second_person(r["task"]) for r in matches[:3])
                return ToolResult(False, f"That matches {len(matches)} reminders: {names}. "
                                         "Which one?",
                                  follow_up=FollowUp("delete_reminder", "task"))
            ids = {r["id"] for r in matches}
            self._items = [r for r in self._items if r["id"] not in ids]
            self._save()
        if query in _ALL:
            return ToolResult(True, f"Okay, I cleared {len(matches)} reminders.")
        return ToolResult(True, f"Okay, I deleted the reminder to "
                                f"{to_second_person(matches[0]['task'])}.")
