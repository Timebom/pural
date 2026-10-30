import logging
import queue
import wave
from collections.abc import Iterator

import numpy as np
import sounddevice as sd

SAMPLE_RATE = 16_000
CHANNELS = 1
FRAME_SAMPLES = 1280

log = logging.getLogger(__name__)


def list_devices() -> list[dict]:
    """Return every audio device PortAudio can see, as plain dicts."""
    default_in, default_out = sd.default.device
    devices = []

    for index, dev in enumerate(sd.query_devices()):
        devices.append({
            "index":index,
            "name": dev["name"],
            "inputs": dev["max_input_channels"],
            "outputs": dev["max_output_channels"],
            "default_rate": dev["default_samplerate"],
            "is_default_input": index == default_in,
            "is_default_output": index == default_out,
        })
    return devices


# def play(samples: np.ndarray, rate: int) -> None:
#     """Play float32 mono audio and block until it finishes."""
#     sd.play(samples, rate)
#     status = sd.wait()
#     if status:
#         log.warning("playback glitch: %s", status)
def play(samples: np.ndarray, rate: int) -> None:
    """Play float32 mono audio and block until it finishes."""
    # Prefer the device’s native rate when possible; otherwise let PortAudio resample.
    with sd.OutputStream(
        samplerate=rate,
        channels=1,
        dtype="float32",
        # optional: latency="high" or a larger blocksize if you still see rare glitches
    ) as stream:
        # ensure C-contiguous float32 mono
        data = np.ascontiguousarray(samples.reshape(-1, 1), dtype=np.float32)
        underflowed = stream.write(data)
        if underflowed:
            log.warning("playback underflow")


def record(seconds: float, device: int | None = None) -> np.ndarray:
    """Record a fixed time. Returns float32 mono, shape (n,), range [-1, 1]."""
    audio = sd.rec(int(seconds * SAMPLE_RATE), samplerate=SAMPLE_RATE, channels=CHANNELS, dtype="float32", device=device)
    sd.wait()
    return audio[:, 0] # (n, 1) -> (n,)


def frames(device: int | None = None) -> Iterator[np.ndarray]:
    """Yield 80ms mono float32 frames from the mic until the generator is closed."""
    q: queue.Queue[np.ndarray] = queue.Queue(maxsize=100)

    def callback(indata, frame_count, time_info, status) -> None:
        if status:
            print("audio status:", status) # overflow/underflow warnings
        try:
            q.put_nowait(indata[:, 0].copy()) # copy: PortAudio reuses indata
        except queue.Full:
            pass # drop rather that block the audio

    with sd.InputStream(samplerate=SAMPLE_RATE, channels=CHANNELS, dtype="float32",
        blocksize=FRAME_SAMPLES, device=device, callback=callback):
            while True:
                yield q.get()

def chime(freq: float = 880.0, seconds: float = 0.12) -> np.ndarray:
    """A short, soft beep as float32 mono at SAMPLE_RATE."""
    t = np.arange(int(SAMPLE_RATE * seconds)) / SAMPLE_RATE
    tone = np.sin(2 * np.pi * freq * t)
    fade = np.minimum(1.0, np.minimum(t, seconds - t) / 0.01)   # 10 ms in/out
    return (0.3 * tone * fade).astype(np.float32)

def save_wav(path: str, audio: np.ndarray, rate: int = SAMPLE_RATE) -> None:
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype(np.int16)
    with wave.open(path, "wb") as f:
        f.setnchannels(1)      # mono
        f.setsampwidth(2)      # 2 bytes = int16
        f.setframerate(rate)
        f.writeframes(pcm.tobytes())
