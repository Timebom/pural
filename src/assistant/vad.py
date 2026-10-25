from pathlib import Path

import numpy as np
import onnxruntime as ort

from assistant.audio import SAMPLE_RATE

WINDOW = 512 # samples per model call at 16kHz (32ms)
CONTEXT = 64 # samples carried over from the previous window


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
