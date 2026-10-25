import queue
from collections.abc import Iterator

import numpy as np
import sounddevice as sd

SAMPLE_RATE = 16_000
CHANNELS = 1
FRAME_SAMPLES = 1280


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


def play(samples: np.ndarray, rate: int) -> None:
    """Play float32 mono audio and block until it finishes."""
    sd.play(samples, rate)
    sd.wait()


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

# if __name__ == "__main__":
#     for d in list_devices():
#         print(d)
