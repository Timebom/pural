use std::io::{self, Write};
mod assistant;
use assistant::core::rules::build_default_engine;
use assistant::runtime::{build_dispatcher, handle};

fn notify(message: String) {
    print!("\nassistant> {}\nyou> ", message);
    let _ = io::stdout().flush();
}

fn main() {
    let engine = build_default_engine();
    let dispatcher = build_dispatcher(notify);

    println!("Type a command (empty line to quit).");

    loop {
        print!("you> ");
        let _ = io::stdout().flush();

        let mut text = String::new();
        match io::stdin().read_line(&mut text) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let text = text.trim();
                if text.is_empty() {
                    break;
                }
                println!("assistant> {}", handle(text, &engine, &dispatcher));
            }
            Err(_) => break,
        }
    }
}
