from pathlib import Path
from typing import Literal

import yaml
from pydantic import BaseModel, ConfigDict, Field, ValidationError


class ConfigError(Exception):
    """Settings file missing, unreadable, or invalid."""


class _Section(BaseModel):
    model_config = ConfigDict(extra="forbid") # a typo is a key is an error, not ignored


class AudioSettings(_Section):
    input_device: int | None = None # None = system default mic

class VADSettings(_Section):
    path: Path = Path("../models/vad/silero_vad.onnx")
    threshold: float = Field(0.5, ge=0, le=1)
    command_silence: float = Field(0.8, gt=0) # quiet that ends a command
    answer_silence: float = Field(1.2, gt=0) # quiet that ends a answer
    wait_seconds: float = Field(5.0, gt=0) # give up if nobody speaks
    max_seconds: float = Field(15.0, gt=0) # hard cap on on utterance
    preroll_frames: int = Field(4, gt=0) # 80ms frames kept before speech


class WakeSettings(_Section):
    phrase: str = "hey_jarvis"
    threshold: float = Field(0.5, ge=0, le=1)
    cooldown_seconds: float = Field(2.0, ge=0)


class STTSettings(_Section):
    model: str = "base.en"
    download_root: Path = Path("../models/whisper")
    device: str = "cpu"
    compute_type: str = "int8"


class TTSSettings(_Section):
    path: Path = Path("../models/tts/en_GB-alba-medium.onnx")


class LLMSettings(_Section):
    enabled: bool = False # Mode A (rules only) unless turned on
    backend: Literal["ollama"] = "ollama"
    model: str = "llama3.2"
    host: str = "http://localhost:11434"
    temperature: float = Field(0.0, ge=0, le=2) # 0 = repeatable routing
    max_tokens: int = Field(150, gt=0)
    keep_alive: str = "30m"
    timeout: float = Field(60.0, gt=0)
    history_turns: int = Field(4, ge=0)


class WeatherSettings(_Section):
    default_location: str = ""          # e.g. "Mumbai". Empty = ask every time.
    timeout: float = Field(8.0, gt=0)


class SearchSettings(_Section):
    region: str = "us-en"               # "in-en" for India
    backend: str = "auto"               # "duckduckgo" = ask one engine only
    max_results: int = Field(5, ge=1, le=30)
    timeout: int = Field(8, ge=1)


class RemindersSettings(_Section):
    path: Path = Path.home() / ".assistant" / "reminders.json"


class ToolsSettings(_Section):
    online: bool = True                 # weather and web search contact third-party servers
    weather: WeatherSettings = Field(default_factory=WeatherSettings)
    search: SearchSettings = Field(default_factory=SearchSettings)
    reminders: RemindersSettings = Field(default_factory=RemindersSettings)


class Settings(_Section):
    audio: AudioSettings = Field(default_factory=AudioSettings)
    vad: VADSettings = Field(default_factory=VADSettings)
    wake: WakeSettings = Field(default_factory=WakeSettings)
    stt: STTSettings = Field(default_factory=STTSettings)
    tts: TTSSettings = Field(default_factory=TTSSettings)
    llm: LLMSettings = Field(default_factory=LLMSettings)
    tools: ToolsSettings = Field(default_factory=ToolsSettings)


def load_settings(path: str | Path | None = None) -> Settings:
    """Defaults, overridden by a YAML file. With no path, uses ./config.yaml if it exists."""
    if path is None:
        if not Path("config.yaml").exists():
            return Settings()
        path = "config.yaml"
    file = Path(path)
    if not file.exists():
        raise ConfigError(f"settings file not found: {file}")
    try:
        data = yaml.safe_load(file.read_text(encoding="utf-8")) or {}
        return Settings.model_validate(data)
    except (yaml.YAMLError, ValidationError) as e:
        raise ConfigError(f"{file}: {e}") from e
