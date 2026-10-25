import io
import wave
from pathlib import Path

import numpy as np
from piper import PiperVoice


class PiperTTS:
    """Satisfies the TTS protocol: synthesize(text) -> (float32 samples, rate)."""

    def __init__(self, model_path: str | Path) -> None:
        path = Path(model_path)
        if not path.exists():
            raise FileNotFoundError(f"Piper voice not found: {path}")
        self._voice = PiperVoice.load(str(path)) # slow: load once, reuse

    def synthesize(self, text: str) -> tuple[np.ndarray, int]:
        buffer = io.BytesIO()
        with wave.open(buffer, "wb") as wav_file:
            self._voice.synthesize_wav(text, wav_file)
        buffer.seek(0)
        with wave.open(buffer, "rb") as wav_file:
            rate = wav_file.getframerate()
            pcm = np.frombuffer(wav_file.readframes(wav_file.getnframes()), dtype=np.int16)
        return pcm.astype(np.float32) / 32768.0, rate
