import logging
import time
from collections.abc import Callable
from contextlib import closing
from itertools import islice

from assistant.audio import frames
from assistant.core.dispatcher import Dispatcher
from assistant.interfaces import STT
from assistant.tools import calculator, clock, fun
from assistant.tools.timer import TimerManager
from assistant.tools.volume import VolumeControl, get_backend
from assistant.vad import SileroVAD, record_until_silence
from assistant.wakeword import WakeWordDetector, wait_for_wakeword

SETTLE_FRAMES = 3

log = logging.getLogger(__name__)


def build_dispatcher(notify: Callable[[str], None]) -> Dispatcher:
    d = Dispatcher()
    timers = TimerManager(on_fire=notify)
    volume = VolumeControl(get_backend())
    d.register("get_time", clock.get_time)
    d.register("get_date", clock.get_date)
    d.register("set_timer", timers.set_timer)
    d.register("calculate", calculator.calculate)
    d.register("tell_joke", fun.tell_joke)
    d.register("set_volume", volume.set_volume)
    d.register("volume_up", volume.volume_up)
    d.register("volume_down", volume.volume_down)
    return d


# def handle(text: str, engine: IntentEngine, dispatcher: Dispatcher) -> str:
#     """One full turn: text in, speakable reply out."""
#     intent = engine.parse(text)
#     if intent is None:
#         return "Sorry, I didn't catch that."
#     return dispatcher.run(intent).message


def _transcribe(clip, stt: STT) -> str:
    start = time.perf_counter()
    text = stt.transcribe(clip)
    log.info("transcribed %.1f seconds of audio in %.2f seconds: %r",
        len(clip) / 16_000, time.perf_counter() - start, text)
    return text

def listen_once(vad: SileroVAD, stt: STT) -> str:
    """Capture one utterance from the mic and return its text ('' if nobody spoke)."""
    with closing(frames()) as source: # guarantees the mic is released
        clip = record_until_silence(vad, source)
    return "" if clip is None else _transcribe(clip, stt)


def wake_turn(
    detector: WakeWordDetector,
    vad: SileroVAD,
    stt: STT,
    on_wake: Callable[[], None]
) -> str:
    """Block until the wake phrase, then capture one command on the same mic stream."""
    detector.reset()
    with closing(frames()) as source:
        if not wait_for_wakeword(detector, source):
            return ""
        on_wake()
        for _ in islice(source, SETTLE_FRAMES):
            pass
        clip = record_until_silence(vad, source)
    return "" if clip is None else _transcribe(clip, stt)
