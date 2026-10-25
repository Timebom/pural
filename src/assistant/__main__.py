from assistant.core.rules import build_default_engine
from assistant.runtime import build_dispatcher, handle
from assistant.speaker import Speaker
from assistant.tts import PiperTTS

TTS_MODEL = "../models/tts/en_GB-alba-medium.onnx"


def main() -> None:
    speaker = Speaker(PiperTTS(TTS_MODEL))

    def notify(message: str) -> None:
        print(f"\nassistant> {message}\nyou> ", end="", flush=True)

    engine = build_default_engine()
    dispatcher = build_dispatcher(notify)
    print("Type a command (empty line to quit).")
    while True:
        try:
            text = input("you> ").strip()
        except (EOFError, KeyboardInterrupt):
            break
        if not text:
            break
        reply = handle(text, engine, dispatcher)
        print("assistant>", reply)
        speaker.say(reply)


if __name__ == "__main__":
    main()
