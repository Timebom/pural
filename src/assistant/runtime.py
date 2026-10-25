from collections.abc import Callable

from assistant.core.dispatcher import Dispatcher
from assistant.interfaces import IntentEngine
from assistant.tools import calculator, clock, fun
from assistant.tools.timer import TimerManager
from assistant.tools.volume import VolumeControl, get_backend


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


def handle(text: str, engine: IntentEngine, dispatcher: Dispatcher) -> str:
    """One full turn: text in, speakable reply out."""
    intent = engine.parse(text)
    if intent is None:
        return "Sorry, I didn't catch that."
    return dispatcher.run(intent).message
