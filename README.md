# Pural

Fully local, modular voice assistant. Wake-word or push-to-talk, offline STT/TTS, rule-based intents + optional small LLM, and practical tools.

**Privacy-first** · Runs entirely on your machine after model download.

## Features

- **Three input modes**
  - Text (type commands)
  - Push-to-talk (`--voice`)
  - Always-listening wake word (`--wake`)
- Rule engine (fast, deterministic) + optional LLM fallback (Ollama)
- Streaming local STT (Faster-Whisper) and TTS (Piper)
- Silero VAD + OpenWakeWord
- Tools: time/date, timers, volume, calculator, reminders, weather, web search, jokes
- Config via YAML + environment variables
- Parallel Rust port in progress

## Quick Start (Python)

```bash
# Clone
git clone https://github.com/Timebom/pural.git
cd pural

# Install (Python ≥ 3.11)
pip install -e .

# Download models (place under models/)
# - TTS: Piper voice (e.g. en_GB-alba-medium.onnx)
# - VAD: silero_vad.onnx
# - Whisper: base.en (or larger)
# - Wake word: OpenWakeWord models

# Run
python -m assistant                  # text mode
python -m assistant --voice          # push-to-talk
python -m assistant --wake           # wake-word mode
python -m assistant --verbose        # timing logs

# Optional Config
python -m assistant --config path/to/config.yaml
```
See src/config.yaml for defaults (wake phrase, VAD thresholds, LLM model, tool settings, etc.).

## Architecture
```Mic → VAD → Wake Word (optional) → Record → STT → Rules / LLM → Tools → TTS → Speaker```
- Rules first (regex + slots) for low latency
- LLM only when rules don't match (and when enabled)
- Tools are registered in a simple dispatcher

## Project Layout
```
src/
├── assistant/               # Python package (main)
│   ├── audio.py
│   ├── vad.py / stt.py / tts.py / wakeword.py / speaker.py
│   ├── config.py
│   ├── runtime.py
│   ├── core/                # rules, intents, conversation, LLM chain
│   ├── tools/               # calculator, clock, timer, volume, weather...
│   └── llm/                 # Ollama backend etc.
├── assistant.rs             # Rust module root
├── main.rs                  # Rust entry point
└── config.yaml
```
Rust side reimplements the same modules (audio, VAD, STT, TTS, rules, tools) using cpal, whisper-rs, piper-rs, oww-rs, etc.

## Tools
| Tool | Description |
|----|------------|
| Time / Date | Current time and date |
| Timer | Set countdown timers |
| Volume | Set / up / down (system volume) |
| Calculator | Basic math expressions |
| Reminders | Create, list, delete reminders |
| Weather | Current / forecast (Open-Meteo) |
| Web Search | DuckDuckGo (when online tools on) |
| Jokes | Simple joke replies |

Online tools (weather, search) can be disabled in config.

## Configuration
All settings live in a Pydantic model loaded from YAML or env.
Key sections:
- `vad` – silence thresholds, wait times
- `wake` – phrase + threshold
- `stt` – model + device
- `llm` – enable + model name
- `tools` – online flag, default location, search region

## Licence
Apache-2.0