import threading
import time
from contextlib import closing

import numpy as np
import sounddevice as sd

from assistant.audio import FRAME_SAMPLES, SAMPLE_RATE, frames, save_wav
from assistant.tts import PiperTTS
from assistant.wakeword import WakeWordDetector

TEXT = "Reminder: call your mom. This is a test of the audio quality."
samples, rate = PiperTTS("../models/tts/en_GB-alba-medium.onnx").synthesize(TEXT)
save_wav("reference.wav", samples, rate)
out = sd.query_devices(kind="output")
print("output device:", out["name"], out["default_samplerate"], "Hz | Piper rate:", rate)
print("Saved reference.wav: open it in QuickTime and listen to it first.")

detector = WakeWordDetector("hey_jarvis")
t0 = time.perf_counter()
for _ in range(50):
    detector.detect(np.zeros(FRAME_SAMPLES, np.float32))
per_frame = (time.perf_counter() - t0) / 50
print(f"detector cost: {per_frame * 1000:.1f} ms of work per 80 ms frame")


def play_callback(x, r):                   # what the app does today: Python callback feeds the card
    sd.play(x, r)
    sd.wait()


def play_blocking(x, r):                   # no Python callback in the audio path
    with sd.OutputStream(samplerate=r, channels=1, dtype="float32") as stream:
        underflowed = stream.write(np.ascontiguousarray(x.reshape(-1, 1)))
    print(f"   blocking write reported underflow: {underflowed}")


def busy(seconds):                         # pure-Python work that holds the GIL
    end = time.perf_counter() + seconds
    while time.perf_counter() < end:
        pass


def load_mic_and_detector(done):
    with closing(frames()) as source:
        for frame in source:
            detector.detect(frame)
            if done.is_set():
                break


def load_detector_only(done):              # no microphone at all
    silence = np.zeros(FRAME_SAMPLES, np.float32)
    while not done.is_set():
        detector.detect(silence)
        time.sleep(FRAME_SAMPLES / SAMPLE_RATE)


def load_mic_and_cpu_burn(done):           # same CPU cost as the detector, but no ONNX
    with closing(frames()) as source:
        for frame in source:
            busy(per_frame)
            if done.is_set():
                break


def run(name, load, play_fn):
    print(f"\n{name}")
    done = threading.Event()

    def speak():
        try:
            time.sleep(1.5)
            play_fn(samples, rate)
        finally:
            done.set()

    thread = threading.Thread(target=speak)
    thread.start()
    load(done)
    thread.join()
    time.sleep(1)


run("1: mic + detector, callback playback (the failing case)", load_mic_and_detector, play_callback)
run("2: mic + detector, BLOCKING playback", load_mic_and_detector, play_blocking)
run("3: detector only, NO microphone, callback playback", load_detector_only, play_callback)
run("4: mic + pure-Python CPU load, callback playback", load_mic_and_cpu_burn, play_callback)
