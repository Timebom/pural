use std::collections::HashMap;
use std::process::Command;

use regex::Regex;
use std::sync::OnceLock;

use crate::core::parsing::parse_number;
use crate::interfaces::{FollowUp, ToolResult};

const STEP: i32 = 10;


pub trait VolumeBackend: Send + Sync {
    fn get(&self) -> Result<i32, String>;
    fn set(&self, level: i32) -> Result<(), String>;
}

fn run(cmd: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}


struct PactlBackend;

impl VolumeBackend for PactlBackend {
    fn get(&self) -> Result<i32, String> {
        let out = run(&["pactl", "get-sink-volume", "@DEFAULT_SINK@"])?;
        let re = Regex::new(r"(\d+)%").unwrap();
        let caps = re
            .captures(&out)
            .ok_or_else(|| "could not read volume".to_string())?;
        caps[1]
            .parse()
            .map_err(|_| "could not parse volume".to_string())
    }

    fn set(&self, level: i32) -> Result<(), String> {
        run(&[
            "pactl",
            "set-sink-volume",
            "@DEFAULT_SINK@",
            &format!("{level}%"),
        ])
        .map(|_| ())
    }
}

struct AmixerBackend;

impl VolumeBackend for AmixerBackend {
    fn get(&self) -> Result<i32, String> {
        let out = run(&["amixer", "get", "Master"])?;
        let re = Regex::new(r"\[(\d+)%\]").unwrap();
        let caps = re
            .captures(&out)
            .ok_or_else(|| "could not read volume".to_string())?;
        caps[1]
            .parse()
            .map_err(|_| "could not parse volume".to_string())
    }

    fn set(&self, level: i32) -> Result<(), String> {
        run(&["amixer", "set", "Master", &format!("{level}%")]).map(|_| ())
    }
}

struct MacBackend;

impl VolumeBackend for MacBackend {
    fn get(&self) -> Result<i32, String> {
        let out = run(&[
            "osascript",
            "-e",
            "output volume of (get volume settings)",
        ])?;
        out.trim()
            .parse()
            .map_err(|_| "could not parse volume".to_string())
    }

    fn set(&self, level: i32) -> Result<(), String> {
        run(&[
            "osascript",
            "-e",
            &format!("set volume output volume {level}"),
        ])
        .map(|_| ())
    }
}


pub fn get_backend() -> Option<Box<dyn VolumeBackend>> {
    let system = std::env::consts::OS;

    match system {
        "linux" => {
            if which("pactl") {
                Some(Box::new(PactlBackend))
            } else if which("amixer") {
                Some(Box::new(AmixerBackend))
            } else {
                None
            }
        }
        "macos" => Some(Box::new(MacBackend)),
        _ => None, // Windows not implemented yet
    }
}


pub struct VolumeControl {
    backend: Option<Box<dyn VolumeBackend>>,
}

impl VolumeControl {
    pub fn new(backend: Option<Box<dyn VolumeBackend>>) -> Self {
        Self { backend }
    }

    fn change<F>(&self, compute: F) -> ToolResult
    where
        F: FnOnce(&dyn VolumeBackend) -> Result<i32, String>,
    {
        let Some(backend) = &self.backend else {
            return ToolResult::err("I can't control the volume on this system yet.");
        };

        match (|| {
            let level = compute(backend.as_ref())?;
            let level = level.clamp(0, 100);
            backend.set(level)?;
            Ok::<_, String>(level)
        })() {
            Ok(level) => {
                let mut data = HashMap::new();
                data.insert("level".to_string(), level.to_string());
                ToolResult::ok(format!("Volume {level} percent.")).with_data(data)
            }
            Err(_) => ToolResult::err("Sorry, I couldn't change the volume."),
        }
    }

    pub fn set_volume(&self, slots: &HashMap<String, String>) -> ToolResult {
        let level = parse_number(slots.get("level").map(|s| s.as_str()).unwrap_or(""));
        let Some(level) = level else {
            return ToolResult::err("What volume level?")
                .with_follow_up(FollowUp::new("set_volume", "level"));
        };

        self.change(|_| Ok(level))
    }

    pub fn volume_up(&self, _slots: &HashMap<String, String>) -> ToolResult {
        self.change(|b| Ok(b.get()? + STEP))
    }

    pub fn volume_down(&self, _slots: &HashMap<String, String>) -> ToolResult {
        self.change(|b| Ok(b.get()? - STEP))
    }
}
