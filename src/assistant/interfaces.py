from dataclasses import dataclass, field
from typing import Protocol

import numpy as np


@dataclass(frozen=True)
class Intent:
    """What the user wants, independent of how we figured it out."""
    name: str
    slots: dict[str, str] = field(default_factory=dict)
    confidence: float = 1.0


@dataclass(frozen=True)
class ToolResult:
    """What a tool returns: structured for code, readable for TTS"""
    ok: bool
    message: str
    data: dict = field(default_factory=dict)


class STT(Protocol):
    def transcribe(self, audio: np.ndarray) -> str:
        """audio: float32, mono, 16khz, values in [-1,1]."""
        ...


class TTS(Protocol):
    def synthesize(self, text: str) -> tuple[np.ndarray, int]:
        """Return (float32 mono samples, sample_rate)."""
        ...


class IntentEngine(Protocol):
    def parse(self, text: str) -> Intent | None:
        """Return an Intent, or None if the text isn't understood."""
        ...
