import argparse
import logging

from assistant.audio import SAMPLE_RATE, chime, play
from assistant.core.conversation import Conversation
from assistant.core.rules import build_default_engine
from assistant.runtime import build_dispatcher
from assistant.speaker import Speaker
from assistant.tts import PiperTTS

ANSWER_SILENCE = 1.2
TTS_MODEL = "../models/tts/en_GB-alba-medium.onnx"
VAD_MODEL = "../models/vad/silero_vad.onnx"
WAKE_PHRASE = "hey_jarvis"


def main() -> None:
    parser = argparse.ArgumentParser(prog="assistant")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--voice", action="store_true",
        help="push-to-talk")
    mode.add_argument("--wake", action="store_true", help="always listening")
    parser.add_argument("--verbose", action="store_true", help="show timing logs")
    args = parser.parse_args()
    logging.basicConfig(
        level=logging.INFO if args.verbose else logging.WARNING,
        format="%(asctime)s %(name)s: %(message)s"
    )

    speaker = Speaker(PiperTTS(TTS_MODEL))

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
        vad = SileroVAD(VAD_MODEL)
        stt = FasterWhisperSTT("base.en", download_root="../models/whisper")
        if args.wake:
            detector = WakeWordDetector(WAKE_PHRASE)
            print(f"Say '{WAKE_PHRASE.replace('_', ' ')}'. Ctrl+C to quit.")
        else:
            print("Press Enter to talk. Ctrl+C to quit.")
    else:
        print("Type a command (empty line to quit).")

    def get_command() -> str | None:
        """Next command. '' = heard nothing (keep going), None = quit."""
        if args.wake:
            return wake_turn(detector, vad, stt,
                on_wake=lambda: play(chime(), SAMPLE_RATE))
        if args.voice:
            input("[Enter to talk]")
            print("listening...")
            return listen_once(vad, stt)
        return input("you> ").strip() or None

    def get_answer() -> str:
        """Answer to a question we just asked: no wake word, more patient than a command."""
        if voice_mode:
            print("listening...")
            return listen_once(vad, stt, silence_seconds=ANSWER_SILENCE)
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
