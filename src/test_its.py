from assistant.audio import frames, save_wav, SAMPLE_RATE
from assistant.speaker import Speaker
from assistant.tts import PiperTTS
from assistant.vad import SileroVAD, record_until_silence
from assistant.stt import FasterWhisperSTT

speaker = Speaker(PiperTTS("models/tts/en_US-lessac-medium.onnx"))
vad = SileroVAD("models/vad/silero_vad.onnx")
stt = FasterWhisperSTT("base.en")

for attempt in range(3):
    speaker.say("For how long?")
    clip = record_until_silence(vad, frames())
    if clip is None:
        print(attempt, "nothing captured")
        continue
    save_wav(f"clip{attempt}.wav", clip)
    print(attempt, f"{len(clip) / SAMPLE_RATE:.1f} s", repr(stt.transcribe(clip)))
