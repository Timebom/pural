import argparse
import logging
import sys

from assistant.audio import SAMPLE_RATE, chime, play
from assistant.config import ConfigError, load_settings
from assistant.core.conversation import Conversation
from assistant.core.rules import build_default_engine
from assistant.runtime import build_dispatcher
from assistant.speaker import Speaker
from assistant.tts import PiperTTS

# ANSWER_SILENCE = 1.2
# TTS_MODEL = "../models/tts/en_GB-alba-medium.onnx"
# VAD_MODEL = "../models/vad/silero_vad.onnx"
# WAKE_PHRASE = "hey_jarvis"


def main() -> None:
    parser = argparse.ArgumentParser(prog="assistant")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--voice", action="store_true",
        help="push-to-talk")
    mode.add_argument("--wake", action="store_true", help="always listening")
    parser.add_argument("--config", help="path to a YAML settings file")
    parser.add_argument("--verbose", action="store_true", help="show timing logs")
    args = parser.parse_args()
    logging.basicConfig(
        level=logging.INFO if args.verbose else logging.WARNING,
        format="%(asctime)s %(name)s: %(message)s"
    )

    try:
        cfg = load_settings(args.config)
    except ConfigError as e:
        sys.exit(f"Could not load settings: {e}")

    speaker = Speaker(PiperTTS(cfg.tts.path))

    def notify(message: str) -> None:
        print(f"\nassistant> {message}")
        speaker.say(message)

    engine = build_default_engine()
    dispatcher = build_dispatcher(notify)
    conversation = Conversation(engine, dispatcher)
    voice_mode = args.voice or args.wake

    if voice_mode: # heavy imports only when needed
        from assistant.runtime import listen_once, wake_turn
        from assistant.stt import FasterWhisperSTT
        from assistant.vad import SileroVAD
        from assistant.wakeword import WakeWordDetector
        device = cfg.audio.input_device
        record_opts = {
            "wait_seconds":cfg.vad.wait_seconds,
            "max_seconds":cfg.vad.max_seconds,
            "preroll_frames":cfg.vad.preroll_frames
        }
        vad = SileroVAD(cfg.vad.path, threshold=cfg.vad.threshold)
        stt = FasterWhisperSTT(cfg.stt.model, cfg.stt.download_root,
            cfg.stt.device, cfg.stt.compute_type)
        if args.wake:
            detector = WakeWordDetector(cfg.wake.phrase, cfg.wake.threshold,
                cfg.wake.cooldown_seconds)
            print(f"Say '{cfg.wake.phrase.replace('_', ' ')}'. Ctrl+C to quit.")
        else:
            print("Press Enter to talk. Ctrl+C to quit.")
    else:
        print("Type a command (empty line to quit).")

    def get_command() -> str | None:
        """Next command. '' = heard nothing (keep going), None = quit."""
        if args.wake:
            return wake_turn(detector, vad, stt,
                on_wake=lambda: play(chime(), SAMPLE_RATE),
                silence_seconds=cfg.vad.command_silence, **record_opts)
        if args.voice:
            input("[Enter to talk]")
            print("listening...")
            return listen_once(vad, stt, device=device,
                silence_seconds=cfg.vad.answer_silence, **record_opts)
        return input("you> ").strip() or None

    def get_answer() -> str:
        """Answer to a question we just asked: no wake word, more patient than a command."""
        if voice_mode:
            print("listening...")
            return listen_once(vad, stt, device=device,
                silence_seconds=cfg.vad.answer_silence, **record_opts)
        return input("you> ").strip()

    while True:
        try:
            text = get_command()
            if text is None:
                break
            if not text:
                print("(heard nothing)")
                continue
            if voice_mode:
                print("you>", text)
            reply = conversation.respond(text)
            while True:
                print("assistant>", reply.text)
                speaker.say(reply.text)
                if not reply.awaiting_answer:
                    break
                answer = get_answer()
                if not answer:
                    conversation.cancel()
                    print("(heard nothing)")
                    break
                if voice_mode:
                    print("you>", answer)
                reply = conversation.respond(answer)
        except (EOFError, KeyboardInterrupt):
            break


if __name__ == "__main__":
    main()
