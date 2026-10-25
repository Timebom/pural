# Pure Python Siri-like Voice Assistant

Fully offline, modular, privacy-first voice assistant written entirely in Python.  
Wake-word activated, local STT + TTS, rule-based + optional lightweight LLM intent handling, and a set of practical tools.

**Target:** Linux (primary), Windows, macOS, Android (Termux/BeeWare)  
**Hardware:** Any GPU + CPU fallback

## Goals

- Always-listening wake-word assistant that feels close to classic Siri
- 100% local after initial model download
- Modular packages that can be used independently
- Two modes: fast deterministic (No-LLM) and natural language (small LLM)
- Trainable components (wake word, optional LLM fine-tune)

## Non-Goals (v1)

- Exact Apple Siri APIs or proprietary models
- Cloud dependency for core features
- Multi-user / enterprise auth
- App Store distribution

## Architecture
```
Microphone → VAD → Wake Word → Record until silence → STT → Intent/LLM → Tools → TTS → Speaker
```
Conversation history kept in memory (optional SQLite persistence).

### Core Modules

| Module                    | Responsibility                          | Key Dependencies              |
|---------------------------|-----------------------------------------|-------------------------------|
| `pural.audio`   | Capture, playback, streaming            | sounddevice, numpy            |
| `pural.vad`     | Voice Activity Detection                | silero-vad / onnxruntime      |
| `pural.wakeword`| Custom wake-word detection              | openwakeword                  |
| `pural.stt`     | Speech-to-Text (streaming preferred)    | faster-whisper                |
| `pural.tts`     | Text-to-Speech with streaming           | piper-tts                     |
| `pural.core`    | Intent parsing + LLM + tools            | ollama / llama-cpp-python     |
| `pural.tools`   | Individual actions                      | stdlib + optional APIs        |
| `pural.runtime` | Main event loop & orchestration         | asyncio                       |
| `pural.config`  | Settings, paths, model locations        | pydantic / yaml               |

### Operating Modes

**Mode A – No-LLM (Classic)**  
Regex + fuzzy matching + finite-state machine. Fast, deterministic, low latency. Implemented first.

**Mode B – Lightweight LLM**  
Ollama or llama-cpp-python with small quantized models (Phi-4-mini, Gemma-2 2B, Llama-3.2 1B/3B). Natural language + tool calling. Falls back to Mode A if model unavailable.

## Technology Stack

| Layer          | Choice                          | Notes                              |
|----------------|---------------------------------|------------------------------------|
| Language       | Python 3.11 / 3.12              | ROCm support                       |
| Wake Word      | OpenWakeWord                    | Trainable, lightweight             |
| VAD            | Silero VAD (ONNX)               | Fast & accurate                    |
| STT            | Faster-Whisper (CTranslate2)    | Best local speed/accuracy          |
| LLM Runtime    | Ollama or llama-cpp-python      | GGUF; ROCm via Ollama possible     |
| LLM Models     | Phi-4-mini / Gemma-2-2B Q4      | Strong small models, low VRAM      |
| TTS            | Piper TTS                       | Offline, high quality, streaming   |
| Audio I/O      | sounddevice + numpy             | Cross-platform, low latency        |
| CLI            | Typer + Rich                    | Modern terminal UX                 |
| GUI (optional) | CustomTkinter or Dear PyGui     | Lightweight desktop UI             |
| Config         | Pydantic Settings + YAML        | Type-safe                          |
| Persistence    | SQLite (optional)               | Session history / memory           |
| Packaging      | PyInstaller / Nuitka / AppImage | Single-file or portable            |

Pure-CPU path is always kept working for reliability.

## Project Structure
```
siri-python-clone/
├── pyproject.toml
├── requirements.txt
├── requirements-training.txt
├── README.md
├── src/
│   └── voice_assistant/
│       ├── init.py
│       ├── audio.py
│       ├── vad.py
│       ├── wakeword.py
│       ├── stt.py
│       ├── tts.py
│       ├── config.py
│       ├── core/
│       │   ├── intents.py      # No-LLM engine
│       │   ├── llm.py          # LLM + function calling
│       │   └── memory.py
│       ├── tools/
│       │   ├── timer.py
│       │   ├── weather.py
│       │   ├── system.py
│       │   ├── media.py
│       │   └── ...
│       └── runtime.py
├── cli/
│   └── main.py
├── gui/                        # Phase 2
│   └── app.py
├── training/
│   ├── train_wakeword.py
│   ├── fine_tune_llm.py
│   ├── quantize.py
│   └── datasets/
├── models/                     # .gitignore
│   ├── wakeword/
│   ├── whisper/
│   ├── llm/
│   └── tts/
├── tests/
└── scripts/
├── download_models.py
└── package.sh
```

## Tools (Initial Set)

- Timer & Alarm
- Weather (Open-Meteo + offline cache)
- System volume / brightness
- Media control (MPRIS on Linux)
- Calculator
- Jokes / fun responses
- Reminders
- Time / Date
- Web search fallback
- Calendar view
- Shutdown / Suspend (with confirmation)

## Configuration

All settings centralized in a Pydantic model loaded from YAML or environment variables.  
Supports profiles (quiet office, noisy environment, low-power) that adjust thresholds and model sizes.

## License

Apache-2.0
