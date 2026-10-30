import logging
import time
from collections.abc import Callable
from contextlib import closing
from itertools import islice

from assistant.audio import frames
from assistant.config import ToolsSettings
from assistant.core.dispatcher import Dispatcher
from assistant.interfaces import STT, ToolResult
from assistant.tools import calculator, chat, clock, fun
from assistant.tools.reminders import ReminderService
from assistant.tools.search import WebSearch
from assistant.tools.timer import TimerManager
from assistant.tools.volume import VolumeControl, get_backend
from assistant.tools.weather import WeatherTool
from assistant.vad import SileroVAD, record_until_silence
from assistant.wakeword import WakeWordDetector, wait_for_wakeword

SETTLE_FRAMES = 3

log = logging.getLogger(__name__)


def build_dispatcher(notify: Callable[[str], None], tools_cfg: ToolsSettings | None = None) -> Dispatcher:
    cfg = tools_cfg or ToolsSettings()
    d = Dispatcher()
    timers = TimerManager(on_fire=notify)
    volume = VolumeControl(get_backend())
    reminders = ReminderService(cfg.reminders.path.expanduser(), on_fire=notify)
    reminders.start()
    d.register("get_time", clock.get_time)
    d.register("get_date", clock.get_date)
    d.register("set_timer", timers.set_timer)
    d.register("calculate", calculator.calculate)
    d.register("tell_joke", fun.tell_joke)
    d.register("set_volume", volume.set_volume)
    d.register("volume_up", volume.volume_up)
    d.register("volume_down", volume.volume_down)
    d.register("chat_reply", chat.chat_reply)
    d.register("set_reminder", reminders.set_reminder)
    d.register("list_reminders", reminders.list_reminders)
    d.register("delete_reminder", reminders.delete_reminder)
    if cfg.online:
        weather = WeatherTool(cfg.weather.default_location, cfg.weather.timeout)
        search = WebSearch(cfg.search.region, cfg.search.backend,
            cfg.search.max_results, cfg.search.timeout)
        d.register("get_weather", weather.get_weather)
        d.register("web_search", search.web_search)
    else:
        def offline(slots: dict[str, str]) -> ToolResult:
            return ToolResult(False, "Online tools are turned off in my settings.")
        d.register("get_weather", offline)
        d.register("web_search", offline)
    return d


def _transcribe(clip, stt: STT) -> str:
    start = time.perf_counter()
    text = stt.transcribe(clip)
    log.info("transcribed %.1f seconds of audio in %.2f seconds: %r",
        len(clip) / 16_000, time.perf_counter() - start, text)
    return text


def listen_once(vad: SileroVAD, stt: STT, device: int | None = None, **record_opts) -> str:
    """Capture one utterance now. A longer silence_seconds tolerates thinking pauses."""
    with closing(frames(device)) as source:
        clip = record_until_silence(vad, source, **record_opts)
    return "" if clip is None else _transcribe(clip, stt)


def wake_turn(
    detector: WakeWordDetector,
    vad: SileroVAD,
    stt: STT,
    on_wake: Callable[[], None],
    device: int | None = None,
    **record_opts
) -> str:
    """Block until the wake phrase, then capture one command on the same mic stream."""
    detector.reset()
    with closing(frames(device)) as source:
        if not wait_for_wakeword(detector, source):
            return ""
        on_wake()
        for _ in islice(source, SETTLE_FRAMES):
            pass
        clip = record_until_silence(vad, source, **record_opts)
    return "" if clip is None else _transcribe(clip, stt)
