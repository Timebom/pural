from collections.abc import Iterable

import numpy as np
from openwakeword.model import Model

from assistant.audio import FRAME_SAMPLES, SAMPLE_RATE


def to_int16(frame: np.ndarray) -> np.ndarray:
    """float32 in [-1, 1] -> int16, clipped so load audio can't wrap around."""
    return (np.clip(frame, -1.0, 1.0) * 32767).astype(np.int16)


class WakeWordDetector:
    def __init__(
        self,
        model: str = "hey_jarvis",
        threshold: float = 0.5,
        cooldown_seconds: float = 2.0,
    ) -> None:
        self._model = Model(wakeword_models=[model], inference_framework="onnx")
        self.threshold = threshold
        self._cooldown_frames = round(cooldown_seconds * SAMPLE_RATE / FRAME_SAMPLES)
        self._cooldown = 0

    def score(self, frame: np.ndarray) -> float:
        """Wake-word probability for one 80ms frame (always updates the model)."""
        scores = self._model.predict(to_int16(frame))
        return float(max(scores.values())) if scores else 0.0

    def detect(self, frame: np.ndarray) -> bool:
        """True once per utterance of the wake phrase."""
        s = self.score(frame)
        if self._cooldown > 0:
            self._cooldown -= 1
            return False
        if s >= self.threshold:
            self._cooldown = self._cooldown_frames
            return True
        return False

    def reset(self) -> None:
        self._cooldown = 0
        self._model.reset()


def wait_for_wakeword(detector: WakeWordDetector, source: Iterable[np.ndarray]) -> bool:
    """Consume frames until the wake phrase is heard. False if the stream ended."""
    for frame in source:
        if detector.detect(frame):
            return True
    return False
