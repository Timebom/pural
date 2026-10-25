from collections import deque
from collections.abc import Iterable
from pathlib import Path

import numpy as np
import onnxruntime as ort

from assistant.audio import FRAME_SAMPLES, SAMPLE_RATE

WINDOW = 512 # samples per model call at 16kHz (32ms)
CONTEXT = 64 # samples carried over from the previous window
FRAME_SECONDS = FRAME_SAMPLES / SAMPLE_RATE # 0.08 seconds


class SileroVAD:
    def __init__(self, model_path: str | Path, threshold: float = 0.5) -> None:
        path = Path(model_path)
        if not path.exists():
            raise FileNotFoundError(f"VAD model not found: {path}")
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = 1
        opts.inter_op_num_threads = 1
        self._session = ort.InferenceSession(
            str(path), sess_options=opts, providers=["CPUExecutionProvider"]
        )
        self.threshold = threshold
        self.reset()

    def reset(self) -> None:
        """Forget all audio history. Call between separate utterances."""
        self._state = np.zeros((2, 1, 128), dtype=np.float32)
        self._context = np.zeros((1, CONTEXT), dtype=np.float32)
        self._pending = np.zeros(0, dtype=np.float32)
        self._last_prob = 0.0

    def _run_window(self, window: np.ndarray) -> float:
        x = np.concatenate([self._context, window[None, :]], axis=1) # (1, 576)
        out, self._state = self._session.run(
            None, {"input": x, "state": self._state,
                "sr": np.array(SAMPLE_RATE, dtype=np.int64)}
        )
        self._context = x[:, -CONTEXT:]
        return float(out[0, 0])

    def get_speech_probability(self, chunk: np.ndarray) -> float:
        """Feed any amount of float32 mono audio; returns the highest speech probability
        among the complete 512-sample windows it contained."""
        self._pending = np.concatenate([self._pending, chunk.astype(np.float32, copy=False)])
        probs = []
        while len(self._pending) >= WINDOW:
            probs.append(self._run_window(self._pending[:WINDOW]))
            self._pending = self._pending[WINDOW:]
        if probs:
            self._last_prob = max(probs)
        return self._last_prob

    def is_speech(self, chunk: np.ndarray) -> bool:
        return self.get_speech_probability(chunk) >= self.threshold

def record_until_silence(
    vad: SileroVAD,
    source: Iterable[np.ndarray],
    wait_seconds: float = 5.0, # give up if nobody speaks
    silence_seconds: float = 0.8, # quiet this long = utterance finished
    max_seconds: float = 15.0, # hard cap on one command
    preroll_frames: int = 4, # 4 x 80ms of audio kept from befor speech
) -> np.ndarray | None:
    """Capture one utterance. Return float32 mono audio, or None if nobody spoke."""
    vad.reset()
    preroll: deque[np.ndarray] = deque(maxlen=preroll_frames)
    collected: list[np.ndarray] = []
    speaking = False
    waited = 0
    silent = 0

    for frame in source:
        is_speech = vad.is_speech(frame)

        if not speaking:
            if is_speech:
                speaking = True
                collected = [*preroll, frame] # include the lead-in
            else:
                preroll.append(frame)
                waited += 1
                if waited * FRAME_SECONDS >= wait_seconds:
                    return None
            continue

        collected.append(frame)
        silent = 0 if is_speech else silent + 1
        if (silent * FRAME_SECONDS >= silence_seconds
            or len(collected) * FRAME_SECONDS >= max_seconds):
            break

    return np.concatenate(collected) if collected else None
