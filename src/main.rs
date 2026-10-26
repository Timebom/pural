use std::io::{self, Write};
use std::path::PathBuf;

use clap::Parser;
use log::LevelFilter;

pub mod assistant;

use crate::assistant::core::rules::build_default_engine;
use crate::assistant::runtime::{build_dispatcher, handle, listen_once};
use crate::assistant::speaker::Speaker;
use crate::assistant::tts::PiperTTS;
use crate::assistant::vad::SileroVAD;
use crate::assistant::stt::FasterWhisperSTT; // add when you have the STT module

const TTS_MODEL: &str = "../models/tts/en_GB-alba-medium.onnx";
const VAD_MODEL: &str = "../models/vad/silero_vad.onnx";

#[derive(Parser, Debug)]
#[command(name = "assistant", about = "Pure Python → Rust voice assistant")]
struct Args {
    /// Push-to-talk: press Enter, then speak
    #[arg(long)]
    voice: bool,

    /// Show timing logs
    #[arg(long)]
    verbose: bool,
}

fn notify(message: String) {
    print!("\nassistant> {}\nyou> ", message);
    let _ = io::stdout().flush();
}

fn main() {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_level(if args.verbose {
            LevelFilter::Info
        } else {
            LevelFilter::Warn
        })
        .format_timestamp_secs()
        .init();

    let tts = match PiperTTS::new(TTS_MODEL) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("failed to load TTS: {e}");
            return;
        }
    };
    let speaker = Speaker::new(tts);

    let engine = build_default_engine();
    let dispatcher = build_dispatcher(notify);

    // Voice mode resources (only created when needed)
    let mut vad = None;
    let mut stt = None;

    if args.voice {
        match SileroVAD::new(VAD_MODEL, 0.5) {
            Ok(v) => vad = Some(v),
            Err(e) => {
                eprintln!("failed to load VAD: {e}");
                return;
            }
        }

        stt = Some(
            FasterWhisperSTT::new("base.en", PathBuf::from("../models/whisper"))
                .expect("failed to load STT"),
        );

        println!("Press Enter to talk. Ctrl+C to quit.");
    } else {
        println!("Type a command (empty line to quit).");
    }

    loop {
        let text = if args.voice {
            print!("[Enter to talk] ");
            let _ = io::stdout().flush();

            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_err() {
                break;
            }

            println!("listening...");

            let vad = vad.as_mut().unwrap();
            let stt = stt.as_ref().unwrap();

            let text = listen_once(vad, stt);
            // let text = String::new(); // replace with real STT call

            if text.is_empty() {
                println!("(heard nothing)");
                continue;
            }
            println!("you> {}", text);
            text
        } else {
            print!("you> ");
            let _ = io::stdout().flush();

            let mut text = String::new();
            match io::stdin().read_line(&mut text) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        break;
                    }
                    text
                }
                Err(_) => break,
            }
        };

        let reply = handle(&text, &engine, &dispatcher);
        println!("assistant> {}", reply);
        speaker.say(&reply);
    }
}
