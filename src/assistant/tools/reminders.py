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

_FIELDS = {"id", "task", "due", "status"}          # "kind" is optional: older files have none
_PRONOUNS = {"my": "your", "me": "you", "i": "you", "mine": "yours", "myself": "yourself",
             "i'm": "you're", "i'll": "you'll", "i've": "you've", "i'd": "you'd"}
_FILLER = {"the", "my", "a", "an", "that", "about", "to", "reminder", "reminders"}
_ALL = {"all", "every", "everything"}
MISSED_AFTER = 120      # seconds late before a reminder or alarm counts as "missed"
MAX_BATCH = 5           # items announced per check; the rest wait for the next tick


def to_second_person(text: str) -> str:
    """'call my mom' -> 'call your mom'. The user said it about themselves."""
    return " ".join(_PRONOUNS.get(word, word) for word in text.split())


def clock_text(due: datetime) -> str:
    hour12 = due.hour % 12 or 12
    suffix = "AM" if due.hour < 12 else "PM"
    return f"{hour12} {suffix}" if due.minute == 0 else f"{hour12}:{due.minute:02d} {suffix}"


def describe_time(due: datetime, now: datetime) -> str:
    clock = clock_text(due)
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


def _kind(item: dict) -> str:
    return item.get("kind", "reminder")


def _matches(query: str, task: str) -> bool:
    """Every query word must (nearly) match a word in the task: 'dentest' finds 'dentist'."""
    task = task.lower()
    if query in task:
        return True
    task_words = task.split()
    return all(any(difflib.SequenceMatcher(None, w, t).ratio() >= 0.8 for t in task_words)
               for w in query.split())


def _join_tasks(items: list[dict]) -> str:
    return ". ".join(to_second_person(r["task"]) for r in items) + "."


def _same_clock(item: dict, target: datetime) -> bool:
    due = datetime.fromtimestamp(item["due"])
    return (due.hour, due.minute) == (target.hour, target.minute)


def _alarm_times(alarms: list[dict], now: datetime) -> str:
    return ", and ".join(describe_time(datetime.fromtimestamp(r["due"]), now)
                         for r in alarms[:5])


class ReminderService:
    """Reminders and alarms: one store, one scheduler. An alarm is an item with kind='alarm'."""

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
        recent = time.time() - 7 * 86400              # finished items are kept a week
        return [r for r in data if isinstance(r, dict) and _FIELDS <= r.keys()
                and (r["status"] == "pending" or r["due"] >= recent)]

    def _save(self) -> None:                           # caller must hold the lock
        self._path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self._path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self._items, indent=2), encoding="utf-8")
        os.replace(tmp, self._path)                    # atomic: never a half-written file

    def _pending(self, kind: str) -> list[dict]:
        """Pending items of one kind, soonest first. The caller must hold the lock."""
        return sorted((r for r in self._items
                       if r["status"] == "pending" and _kind(r) == kind),
                      key=lambda r: r["due"])

    def _store(self, task: str, due: datetime, kind: str) -> dict | None:
        entry = {"id": uuid.uuid4().hex[:8], "task": task, "due": due.timestamp(),
                 "status": "pending", "kind": kind, "created": time.time()}
        try:
            with self._lock:
                self._items.append(entry)
                self._save()
        except OSError as e:
            log.error("could not save %s: %s", kind, e)
            with self._lock:
                self._items.remove(entry)
            return None
        return entry

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
        """Speak every pending item whose time has come (up to MAX_BATCH per call).
        Returns how many were handled."""
        now = time.time() if now is None else now
        with self._lock:
            due = sorted((r for r in self._items
                          if r["status"] == "pending" and r["due"] <= now),
                         key=lambda r: r["due"])[:MAX_BATCH]
            for r in due:
                r["status"] = "done"                   # mark first: at most once, even if speaking fails
            if due:
                self._save()
        for message in self._fire_messages(due, now):  # speak outside the lock
            self._on_fire(message)
        return len(due)

    @staticmethod
    def _alarm_messages(alarms: list[dict], now: float) -> list[str]:
        on_time = [r for r in alarms if now - r["due"] <= MISSED_AFTER]
        missed = [r for r in alarms if now - r["due"] > MISSED_AFTER]
        messages = [f"Alarm. It's {clock_text(datetime.fromtimestamp(r['due']))}."
                    for r in on_time]
        if len(missed) == 1:
            messages.append(
                f"You missed your {clock_text(datetime.fromtimestamp(missed[0]['due']))} alarm.")
        elif missed:
            messages.append(f"You missed {len(missed)} alarms.")
        return messages

    @staticmethod
    def _fire_messages(due: list[dict], now: float) -> list[str]:
        """Alarms first, then one message for on-time reminders and one for missed ones."""
        messages = ReminderService._alarm_messages(
            [r for r in due if _kind(r) == "alarm"], now)
        reminders = [r for r in due if _kind(r) != "alarm"]
        missed = [r for r in reminders if now - r["due"] > MISSED_AFTER]
        on_time = [r for r in reminders if now - r["due"] <= MISSED_AFTER]
        if len(on_time) == 1:
            messages.append(f"Reminder: {to_second_person(on_time[0]['task'])}.")
        elif on_time:
            messages.append(f"You have {len(on_time)} reminders: {_join_tasks(on_time)}")
        if len(missed) == 1:
            when = describe_time(datetime.fromtimestamp(missed[0]["due"]),
                                 datetime.fromtimestamp(now))
            messages.append(f"You missed a reminder from {when}: "
                            f"{to_second_person(missed[0]['task'])}.")
        elif missed:
            messages.append(f"You missed {len(missed)} reminders: {_join_tasks(missed)}")
        return messages

    @staticmethod
    def _check_time(when: str, now: datetime, ask: FollowUp,
                    question: str) -> tuple[datetime | None, ToolResult | None]:
        """Either a usable future time, or a spoken follow-up question."""
        due = parse_when(when, now)
        if due is None:
            return None, ToolResult(False, f"Sorry, I didn't catch when. {question}",
                                    follow_up=ask)
        if due <= now:
            return None, ToolResult(False, f"That time has already passed. {question}",
                                    follow_up=ask)
        return due, None

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
        question = "When should I remind you?"
        ask_when = FollowUp("set_reminder", "when", {"task": task})
        if not when:
            return ToolResult(False, question, follow_up=ask_when)
        now = datetime.now()
        due, error = self._check_time(when, now, ask_when, question)
        if error:
            return error
        entry = self._store(task, due, "reminder")
        if entry is None:
            return ToolResult(False, "I couldn't save that reminder.")
        return ToolResult(True, f"Okay, I'll remind you to {to_second_person(task)} "
                                f"{describe_time(due, now)}.", {"id": entry["id"]})

    def list_reminders(self, slots: dict[str, str]) -> ToolResult:
        now = datetime.now()
        with self._lock:
            pending = self._pending("reminder")
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
            pending = self._pending("reminder")
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

    def set_alarm(self, slots: dict[str, str]) -> ToolResult:
        when = slots.get("when", "").strip()
        question = "What time should I set it for?"
        ask = FollowUp("set_alarm", "when")
        if not when:
            return ToolResult(False, "What time should I set the alarm for?", follow_up=ask)
        now = datetime.now()
        due, error = self._check_time(when, now, ask, question)
        if error:
            return error
        entry = self._store("alarm", due, "alarm")
        if entry is None:
            return ToolResult(False, "I couldn't save that alarm.")
        return ToolResult(True, f"Okay, alarm set for {describe_time(due, now)}.",
                          {"id": entry["id"]})

    def list_alarms(self, slots: dict[str, str]) -> ToolResult:
        now = datetime.now()
        with self._lock:
            alarms = self._pending("alarm")
        if not alarms:
            return ToolResult(True, "You have no alarms.")
        if len(alarms) == 1:
            when = describe_time(datetime.fromtimestamp(alarms[0]["due"]), now)
            return ToolResult(True, f"You have one alarm, for {when}.", {"count": 1})
        return ToolResult(True, f"You have {len(alarms)} alarms: {_alarm_times(alarms, now)}.",
                          {"count": len(alarms)})

    def cancel_alarm(self, slots: dict[str, str]) -> ToolResult:
        which = normalize(slots.get("which", ""))
        now = datetime.now()
        ask = FollowUp("cancel_alarm", "which")
        with self._lock:
            alarms = self._pending("alarm")
            if not alarms:
                return ToolResult(True, "You have no alarms.")
            overview = f"You have alarms for {_alarm_times(alarms, now)}. Which one?"
            """ previous code:
                if which in _ALL:
                    matches = alarms
                elif not which and len(alarms) == 1:
                    matches = alarms
            """
            if which in _ALL or (not which and len(alarms) == 1):
                matches = alarms
            elif not which:
                return ToolResult(False, f"Which alarm? {overview}", follow_up=ask)
            else:
                target = parse_when(which, now)
                matches = ([] if target is None
                           else [r for r in alarms if _same_clock(r, target)])
                if not matches:
                    return ToolResult(False, f"I couldn't find an alarm for {which}. "
                                             f"{overview}", follow_up=ask)
            ids = {r["id"] for r in matches}
            self._items = [r for r in self._items if r["id"] not in ids]
            self._save()
        if len(matches) > 1:
            return ToolResult(True, f"Okay, I cleared {len(matches)} alarms.")
        when = describe_time(datetime.fromtimestamp(matches[0]["due"]), now)
        return ToolResult(True, f"Okay, I cancelled the alarm for {when}.")
