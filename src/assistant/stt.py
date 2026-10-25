from pathlib import Path

import numpy as np
from faster_whisper import WhisperModel


class FasterWhisperSTT:
    """Satisfies the STT protocol: transcribe(audio) -> str."""

    def __init__(
        self,
        model: str = "base.en",
        download_root: str | Path = "models/whisper",
        device: str = "cpu",
        compute_type: str = "int8",
    ) -> None:
        self._model = WhisperModel(
            model, device=device, compute_type=compute_type, download_root=str(download_root)
        )

    def transcribe(self, audio: np.ndarray) -> str:
        """audio: float32 mono 16kHz in [-1, 1]. Returns plain text ("" if nothing heard)."""
        if audio.size < 1600: # under 0.1 seconds: nothing to transcribe
            return ""
        segments, _info = self._model.transcribe(
            audio, language="en", beam_size=1
        )
        # `segments` is a lazy generator: the real work happens as we iterate.
        return " ".join(seg.text.strip() for seg in segments).strip()
