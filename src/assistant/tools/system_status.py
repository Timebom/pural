import logging
import shutil
from collections.abc import Callable
from pathlib import Path

from assistant.interfaces import ToolResult

log = logging.getLogger(__name__)

_WHAT = {"battery": "battery", "cpu": "cpu", "processor": "cpu",
         "memory": "memory", "ram": "memory", "disk": "disk", "storage": "disk"}


def read_stats() -> dict:
    """Measure this computer. Needs psutil. The battery fields are None on desktops."""
    import psutil
    battery = psutil.sensors_battery() if hasattr(psutil, "sensors_battery") else None
    memory = psutil.virtual_memory()
    disk = shutil.disk_usage(Path.home())
    return {
        "battery_percent": battery.percent if battery else None,
        "plugged_in": battery.power_plugged if battery else None,
        "cpu_percent": psutil.cpu_percent(interval=0.3),      # a real 0.3 s sample
        "memory_percent": memory.percent,
        "memory_free_gb": memory.available / 1e9,
        "disk_percent": disk.used / disk.total * 100,
        "disk_free_gb": disk.free / 1e9,
    }


def _gb(value: float) -> str:
    rounded = round(value, 1)
    return str(round(value)) if rounded >= 10 else str(rounded)


def _battery(s: dict) -> str:
    if s["battery_percent"] is None:
        return "This computer doesn't report a battery."
    text = f"Your battery is at {round(s['battery_percent'])} percent"
    if s["plugged_in"] is True:
        text += " and plugged in"
    elif s["plugged_in"] is False:
        text += " and running on battery"
    return text + "."


def _cpu(s: dict) -> str:
    return f"The processor is at {round(s['cpu_percent'])} percent."


def _memory(s: dict) -> str:
    return (f"Memory is {round(s['memory_percent'])} percent used, "
            f"with {_gb(s['memory_free_gb'])} gigabytes free.")


def _disk(s: dict) -> str:
    return (f"The disk is {round(s['disk_percent'])} percent full, "
            f"with {_gb(s['disk_free_gb'])} gigabytes free.")


_SENTENCES = {"battery": _battery, "cpu": _cpu, "memory": _memory, "disk": _disk}


class SystemStatus:
    def __init__(self, stats: Callable[[], dict] | None = None) -> None:
        self._stats = stats or read_stats

    def system_status(self, slots: dict[str, str]) -> ToolResult:
        what = _WHAT.get(slots.get("what", "").strip().lower())
        try:
            s = self._stats()
        except ImportError:
            return ToolResult(False, "System status needs the psutil package. "
                                     "Install it with pip.")
        except (OSError, RuntimeError, AttributeError) as e:
            log.warning("could not read system status: %s", e)
            return ToolResult(False, "I couldn't read the system status.")
        if what:
            return ToolResult(True, _SENTENCES[what](s), {"stats": s})
        parts = [_cpu(s), _memory(s), _disk(s)]
        if s["battery_percent"] is not None:
            parts.insert(0, _battery(s))             # desktops: don't announce "no battery"
        return ToolResult(True, " ".join(parts), {"stats": s})
