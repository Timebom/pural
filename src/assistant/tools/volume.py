import platform
import re
import shutil
import subprocess
from typing import Protocol

from assistant.interfaces import ToolResult

STEP = 10


class VolumeBackend(Protocol):
    def get(self) -> int: ...
    def set(self, level: int) -> None: ...


def _run(cmd: list[str]) -> str:
    return subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=5).stdout


class PactlBackend:
    """Linux with PulseAudio or PipeWire (pactl works on both)."""
    def get(self) -> int:
        out = _run(["pactl", "get-sink-volume", "@DEFAULT_SINK@"])
        match = re.search(r"(\d+)%", out)
        if not match:
            raise RuntimeError("could not read volume")
        return int(match.group(1))

    def set(self, level: int) -> None:
        _run(["pactl", "set-sink-volume", "@DEFAULT_SINK@", f"{level}%"])


class AmixerBackend:
    """Linux ALSA fallback."""
    def get(self) -> int:
        match = re.search(r"\[(\d+)%\]", _run(["amixer", "get", "Master"]))
        if not match:
            raise RuntimeError("could not read volume")
        return int(match.group(1))

    def set(self, level: int) -> None:
        _run(["amixer", "set", "Master", f"{level}%"])


class MacBackend:
    def get(self) -> int:
        return int(_run(["osascript", "-e", "output volume of (get volume settings)"]))

    def set(self, level: int) -> None:
        _run(["osascript", "-e", f"set volume output volume {level}"])


def get_backend() -> VolumeBackend | None:
    """Pick a backend for this OS, or None if nothing usable is installed."""
    system = platform.system()
    if system == "linux":
        if shutil.which("pactl"):
            return PactlBackend()
        if shutil.which("amixer"):
            return AmixerBackend()
    elif system == "Darwin":
        return MacBackend()
    return None # Windows: not implemented yet


class VolumeControl:
    def __init__(self, backend: VolumeBackend | None) -> None:
        self._backend = backend

    def _change(self, compute) -> ToolResult:
        if self._backend is None:
            return ToolResult(False, "I can't control the volume on this system yet.")
        try:
            level = max(0, min(100, compute(self._backend)))
            self._backend.set(level)
        except (subprocess.SubprocessError, RuntimeError, OSError, ValueError):
            return ToolResult(False, "Sorry, I couldn't change the volume.")
        return ToolResult(True, f"Volume {level} percent.", {"level": level})

    def set_volume(self, slots: dict[str, str]) -> ToolResult:
        try:
            target = int(slots.get("level", ""))
        except ValueError:
            return ToolResult(False, "What volume level?")
        return self._change(lambda b: target)

    def volume_up(self, slots: dict[str, str]) -> ToolResult:
        return self._change(lambda b: b.get() + STEP)

    def volume_down(self, slots: dict[str, str]) -> ToolResult:
        return self._change(lambda b: b.get() - STEP)
