import argparse
import logging

from assistant.core.rules import build_default_engine
from assistant.runtime import build_dispatcher, handle
from assistant.speaker import Speaker
from assistant.tts import PiperTTS

TTS_MODEL = "../models/tts/en_GB-alba-medium.onnx"
VAD_MODEL = "../models/vad/silero_vad.onnx"


def main() -> None:
    parser = argparse.ArgumentParser(prog="assistant")
    parser.add_argument("--voice", action="store_true",
        help="push-to-talk: press Enter, then speak")
    parser.add_argument("--verbose", action="store_true", help="show timing logs")
    args = parser.parse_args()
    logging.basicConfig(
        level=logging.INFO if args.verbose else logging.WARNING,
        format="%(asctime)s %(name)s: %(message)s"
    )

    speaker = Speaker(PiperTTS(TTS_MODEL))

    def notify(message: str) -> None:
        print(f"\nassistant> {message}\nyou> ", end="", flush=True)

    engine = build_default_engine()
    dispatcher = build_dispatcher(notify)

    if args.voice: # heavy imports only when needed
        from assistant.runtime import listen_once
        from assistant.stt import FasterWhisperSTT
        from assistant.vad import SileroVAD
        vad = SileroVAD(VAD_MODEL)
        stt = FasterWhisperSTT("base.en", download_root="../models/whisper")
        print("Press Enter to talk. Ctrl+C to quit.")
    else:
        print("Type a command (empty line to quit).")

    while True:
        try:
            if args.voice:
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
