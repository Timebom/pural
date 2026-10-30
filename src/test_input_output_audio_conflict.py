import threading
import time
from contextlib import closing

import sounddevice as sd

from assistant.audio import frames
from assistant.speaker import Speaker
from assistant.tts import PiperTTS
from assistant.wakeword import WakeWordDetector

sd.default.latency = "high"

speaker = Speaker(PiperTTS("models/tts/en_US-lessac-medium.onnx"))
TEXT = "Reminder: call your mom. This is a test of the audio quality."


def say_later(delay: float) -> threading.Timer:
    t = threading.Timer(delay, speaker.say, args=(TEXT,))
    t.start()
    return t


print("A: main thread, nothing else running")
speaker.say(TEXT)
time.sleep(1)

print("B: background thread, nothing else running")
say_later(0.1).join()
time.sleep(1)

print("C: background thread while the mic stream is open")
t = say_later(1.0)
with closing(frames()) as source:
    for i, _ in enumerate(source):
        if i > 100:                       # about 8 seconds of doing nothing with the mic
            break
t.join()

print("D: background thread, mic open AND the wake-word detector running")
detector = WakeWordDetector("hey_jarvis")
t = say_later(1.0)
with closing(frames()) as source:
    for i, frame in enumerate(source):
        detector.detect(frame)
        if i > 100:
            break
t.join()
