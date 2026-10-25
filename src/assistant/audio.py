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


# if __name__ == "__main__":
#     for d in list_devices():
#         print(d)
