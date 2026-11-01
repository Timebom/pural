import logging
import platform
import shutil
import subprocess
from collections.abc import Callable
from typing import Protocol

from assistant.core.intents import normalize
from assistant.interfaces import FollowUp, ToolResult

log = logging.getLogger(__name__)

_YES = {"yes", "yeah", "yep", "yup", "sure", "confirm", "affirmative", "absolutely"}
_NO = {"no", "not", "dont", "don't", "never", "wait", "stop", "cancel", "nope", "nah"}
_YES_PHRASES = {"do it", "go ahead"}


def is_yes(text: str) -> bool:
    """Strict on purpose: only a clear yes counts. Anything else means no."""
    cleaned = normalize(text)
    if cleaned is _YES_PHRASES:
        return True
    words = cleaned.split()
    return bool(words) and words[0] in _YES and not (set(words) & _NO)


class PowerError(Exception):
    """No power command worked."""


class PowerBackend(Protocol):
    def shutdown(self) -> None: ...
    def suspend(self) -> None: ...


def _run_first_working(options: list[list[str]]) -> None:
    errors = []
    for cmd in options:
        if shutil.which(cmd[0]) is None:
            continue
        try:
            subprocess.run(cmd, check=True, timeout=15, capture_output=True)
            return
        except (subprocess.SubprocessError, OSError) as e:
            errors.append(f"{cmd[0]}: {e}")
    raise PowerError("; ".join(errors) or "no supported power command found")


class LinuxPower:
    def shutdown(self) -> None:
        _run_first_working([["systemctl", "poweroff"], ["loginctl", "poweroff"],
            ["shutdown", "-h", "now"]])

    def suspend(self) -> None:
        _run_first_working([["systemctl", "suspend"], ["loginctl", "suspend"],
            ["pm-suspend"]])


class MacPower:
    def shutdown(self) -> None:
        _run_first_working([["osascript", "-e",
            'tell application "System Events" to shut down']])

    def suspend(self) -> None:
        _run_first_working([["pmset", "sleepnow"]])


class WindowsPower:
    def shutdown(self) -> None:
        _run_first_working([["shutdown", "/s", "/t", "0"]])

    def suspend(self) -> None:
        _run_first_working([["rundll32.exe", "powrprof.dll,SetSuspendState", "0,1,0"]])


def get_backend() -> PowerBackend | None:
    return {
        "linux": LinuxPower,
        "Darwin": MacPower,
        "Windows": WindowsPower
    }.get(platform.system(), lambda: None)()


# action -> (what the question says, what is said while doing it, what is said on "no")
_ACTIONS = {
    "shutdown": ("shut down the computer", "Shutting down now.", "Okay, I won't shut down."),
    "suspend": ("put the computer to sleep", "Going to sleep now.", "Okay, staying awake."),
}


class PowerControl:
    def __init__(
        self,
        backend: PowerBackend | None,
        warning: Callable[[], str] | None = None,
        notify: Callable[[str], None] | None = None,
        enabled: bool = True,
        dry_run: bool = False
    ) -> None:
        self._backend = backend
        self._warning = warning
        self._notify = notify or (lambda message: None)
        self._enabled = enabled
        self._dry_run = dry_run

    def shutdown(self, slots: dict[str, str]) -> ToolResult:
        return self._handle("shutdown", slots)

    def suspend(self, slots: dict[str, str]) -> ToolResult:
        return self._handle("suspend", slots)

    def _handle(self, action: str, slots: dict[str, str]) -> ToolResult:
        label, doing, declined = _ACTIONS[action]
        if not self._enabled:
            return ToolResult(False, "Power command are turned off in my settings.")
        if self._backend is None and not self._dry_run:
            return ToolResult(False, "I can't control power on this system yet.")
        answer = slots.get("confirm")
        if answer is None:
            pending = self._warning() if self._warning else ""
            extra = f" You have {pending} that won't go off." if pending else ""
            return ToolResult(False, f"Are you sure you want to {label}?{extra}",
                follow_up=FollowUp(action, "confirm"))

        if not is_yes(answer):
            return ToolResult(True, declined)
        if self._dry_run:
            return ToolResult(True, f"This is a dry run, so I'm not going to {label}.")
        return ToolResult(True, doing, after=lambda: self._execute(action))

    def _execute(self, action: str) -> None:
        """Runs after the words are spoken. Must never raise: nothing is above it to catch it."""
        try:
            getattr(self._backend, action)()
        except (PowerError, subprocess.SubprocessError, OSError) as e:
            log.error("%s failed: %s", action, e)
            self._notify(f"Sorry, I couldn't {_ACTIONS[action][0]}.")
