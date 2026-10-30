use std::collections::HashMap;
use std::process::Command;
use regex::Regex;
use once_cell::sync::Lazy;

use crate::assistant::interfaces::ToolResult;

const STEP: i32 = 10;

static PERCENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+)%").unwrap());
static AMIXER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[(\d+)%\]").unwrap());

/// Common interface for volume backends.
trait VolumeBackend: Send + Sync {
    fn get(&self) -> Result<i32, String>;
    fn set(&self, level: i32) -> Result<(), String>;
}

fn run(cmd: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!("command failed: {:?}", cmd));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------- Linux: PulseAudio / PipeWire ----------

struct PactlBackend;

impl VolumeBackend for PactlBackend {
    fn get(&self) -> Result<i32, String> {
        let out = run(&["pactl", "get-sink-volume", "@DEFAULT_SINK@"])?;
        let caps = PERCENT_RE
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
            &format!("{}%", level),
        ])?;
        Ok(())
    }
}

// ---------- Linux: ALSA fallback ----------

struct AmixerBackend;

impl VolumeBackend for AmixerBackend {
    fn get(&self) -> Result<i32, String> {
        let out = run(&["amixer", "get", "Master"])?;
        let caps = AMIXER_RE
            .captures(&out)
            .ok_or_else(|| "could not read volume".to_string())?;
        caps[1]
            .parse()
            .map_err(|_| "could not parse volume".to_string())
    }

    fn set(&self, level: i32) -> Result<(), String> {
        run(&["amixer", "set", "Master", &format!("{}%", level)])?;
        Ok(())
    }
}

// ---------- macOS ----------

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
            &format!("set volume output volume {}", level),
        ])?;
        Ok(())
    }
}

/// Pick a backend for this OS, or `None` if nothing usable is installed.
fn get_backend() -> Option<Box<dyn VolumeBackend>> {
    match std::env::consts::OS {
        "linux" => {
            if which::which("pactl").is_ok() {
                Some(Box::new(PactlBackend))
            } else if which::which("amixer").is_ok() {
                Some(Box::new(AmixerBackend))
            } else {
                None
            }
        }
        "macos" => Some(Box::new(MacBackend)),
        _ => None, // Windows not implemented yet
    }
}

// ---------- High-level controller ----------

pub struct VolumeControl {
    backend: Option<Box<dyn VolumeBackend>>,
}

impl VolumeControl {
    pub fn new() -> Self {
        Self {
            backend: get_backend(),
        }
    }

    fn change<F>(&self, compute: F) -> ToolResult
    where
        F: FnOnce(&dyn VolumeBackend) -> Result<i32, String>,
    {
        let backend = match &self.backend {
            Some(b) => b.as_ref(),
            None => {
                return ToolResult::failure("I can't control the volume on this system yet.");
            }
        };

        match compute(backend) {
            Ok(raw_level) => {
                let level = raw_level.clamp(0, 100);
                match backend.set(level) {
                    Ok(()) => {
                        let mut data = HashMap::new();
                        data.insert("level".to_string(), level.to_string());
                        ToolResult::success(format!("Volume {} percent.", level)).with_data(data)
                    }
                    Err(_) => ToolResult::failure("Sorry, I couldn't change the volume."),
                }
            }
            Err(_) => ToolResult::failure("Sorry, I couldn't change the volume."),
        }
    }

    pub fn set_volume(&self, slots: &HashMap<String, String>) -> ToolResult {
        let target = match slots.get("level").and_then(|s| s.parse::<i32>().ok()) {
            Some(v) => v,
            None => return ToolResult::failure("What volume level?"),
        };

        self.change(|_| Ok(target))
    }

    pub fn volume_up(&self, _slots: &HashMap<String, String>) -> ToolResult {
        self.change(|b| Ok(b.get()? + STEP))
    }

    pub fn volume_down(&self, _slots: &HashMap<String, String>) -> ToolResult {
        self.change(|b| Ok(b.get()? - STEP))
    }
}

impl Default for VolumeControl {
    fn default() -> Self {
        Self::new()
    }
}
