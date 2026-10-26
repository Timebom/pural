import argparse
import logging

from assistant.audio import SAMPLE_RATE, chime, play
from assistant.core.rules import build_default_engine
from assistant.runtime import build_dispatcher, handle
from assistant.speaker import Speaker
from assistant.tts import PiperTTS

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

    if args.voice: # heavy imports only when needed
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

    while True:
        try:
            if args.wake:
                text = wake_turn(detector, vad, stt,
                    on_wake=lambda: play(chime(), SAMPLE_RATE))
                if not text:
                    print("(heard nothing)")
                    continue
                print("you>", text)
            elif args.voice:
                input("[Enter to talk] ")
                print("listening...")
                text = listen_once(vad, stt)
                if not text:
                    print("(heard nothing)")
                    continue
                print("you>", text)
            else:
                text = input("you> ").strip()
                if not text:
                    break
        except (EOFError, KeyboardInterrupt):
            break
        reply = handle(text, engine, dispatcher)
        print("assistant>", reply)
        speaker.say(reply)


if __name__ == "__main__":
    main()
