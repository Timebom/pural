use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("settings file not found: {0}")]
    NotFound(PathBuf),

    #[error("{0}: {1}")]
    Invalid(PathBuf, String),
}

// Nested settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSettings {
    /// `None` = system default microphone
    #[serde(default)]
    pub input_device: Option<i32>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            input_device: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VADSettings {
    #[serde(default = "default_vad_path")]
    pub path: PathBuf,

    #[serde(default = "default_0_5")]
    pub threshold: f64,

    /// Quiet that ends a command
    #[serde(default = "default_0_8")]
    pub command_silence: f64,

    /// Quiet that ends an answer
    #[serde(default = "default_1_2")]
    pub answer_silence: f64,

    /// Give up if nobody speaks
    #[serde(default = "default_5_0")]
    pub wait_seconds: f64,

    /// Hard cap on an utterance
    #[serde(default = "default_15_0")]
    pub max_seconds: f64,

    /// 80 ms frames kept before speech
    #[serde(default = "default_4")]
    pub preroll_frames: u32,
}

fn default_vad_path() -> PathBuf {
    PathBuf::from("../../models/vad/silero_vad.onnx")
}
fn default_0_5() -> f64 {
    0.5
}
fn default_0_8() -> f64 {
    0.8
}
fn default_1_2() -> f64 {
    1.2
}
fn default_5_0() -> f64 {
    5.0
}
fn default_15_0() -> f64 {
    15.0
}
fn default_4() -> u32 {
    4
}

impl Default for VADSettings {
    fn default() -> Self {
        Self {
            path: default_vad_path(),
            threshold: default_0_5(),
            command_silence: default_0_8(),
            answer_silence: default_1_2(),
            wait_seconds: default_5_0(),
            max_seconds: default_15_0(),
            preroll_frames: default_4(),
        }
    }
}

impl VADSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err("vad.threshold must be in [0, 1]".into());
        }
        if self.command_silence <= 0.0 {
            return Err("vad.command_silence must be > 0".into());
        }
        if self.answer_silence <= 0.0 {
            return Err("vad.answer_silence must be > 0".into());
        }
        if self.wait_seconds <= 0.0 {
            return Err("vad.wait_seconds must be > 0".into());
        }
        if self.max_seconds <= 0.0 {
            return Err("vad.max_seconds must be > 0".into());
        }
        if self.preroll_frames == 0 {
            return Err("vad.preroll_frames must be > 0".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WakeSettings {
    #[serde(default = "default_wake_phrase")]
    pub phrase: String,

    #[serde(default = "default_0_5")]
    pub threshold: f64,

    #[serde(default = "default_2_0")]
    pub cooldown_seconds: f64,
}

fn default_wake_phrase() -> String {
    "alexa".into()
}
fn default_2_0() -> f64 {
    2.0
}

impl Default for WakeSettings {
    fn default() -> Self {
        Self {
            phrase: default_wake_phrase(),
            threshold: default_0_5(),
            cooldown_seconds: default_2_0(),
        }
    }
}

impl WakeSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err("wake.threshold must be in [0, 1]".into());
        }
        if self.cooldown_seconds < 0.0 {
            return Err("wake.cooldown_seconds must be >= 0".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct STTSettings {
    #[serde(default = "default_stt_model")]
    pub model: String,

    #[serde(default = "default_stt_download_root")]
    pub download_root: PathBuf,

    #[serde(default = "default_cpu")]
    pub device: String,

    #[serde(default = "default_int8")]
    pub compute_type: String,
}

fn default_stt_model() -> String {
    "base.en".into()
}
fn default_stt_download_root() -> PathBuf {
    PathBuf::from("../../models/whisper")
}
fn default_cpu() -> String {
    "cpu".into()
}
fn default_int8() -> String {
    "int8".into()
}

impl Default for STTSettings {
    fn default() -> Self {
        Self {
            model: default_stt_model(),
            download_root: default_stt_download_root(),
            device: default_cpu(),
            compute_type: default_int8(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TTSSettings {
    #[serde(default = "default_tts_path")]
    pub path: PathBuf,
}

fn default_tts_path() -> PathBuf {
    PathBuf::from("../../models/tts/en_GB-alba-medium.onnx")
}

impl Default for TTSSettings {
    fn default() -> Self {
        Self {
            path: default_tts_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LLMBackend {
    Ollama,
}

impl Default for LLMBackend {
    fn default() -> Self {
        LLMBackend::Ollama
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LLMSettings {
    /// Mode A (rules only) unless turned on
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub backend: LLMBackend,

    #[serde(default = "default_llm_model")]
    pub model: String,

    #[serde(default = "default_llm_host")]
    pub host: String,

    /// 0 = repeatable routing
    #[serde(default)]
    pub temperature: f64,

    #[serde(default = "default_150")]
    pub max_tokens: u32,

    #[serde(default = "default_keep_alive")]
    pub keep_alive: String,

    #[serde(default = "default_60_0")]
    pub timeout: f64,

    #[serde(default = "default_4")]
    pub history_turns: u32,
}

fn default_llm_model() -> String {
    "llama3.2".into()
}
fn default_llm_host() -> String {
    "http://localhost:11434".into()
}
fn default_150() -> u32 {
    150
}
fn default_keep_alive() -> String {
    "30m".into()
}
fn default_60_0() -> f64 {
    60.0
}

impl Default for LLMSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: LLMBackend::default(),
            model: default_llm_model(),
            host: default_llm_host(),
            temperature: 0.0,
            max_tokens: default_150(),
            keep_alive: default_keep_alive(),
            timeout: default_60_0(),
            history_turns: default_4(),
        }
    }
}

impl LLMSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err("llm.temperature must be in [0, 2]".into());
        }
        if self.max_tokens == 0 {
            return Err("llm.max_tokens must be > 0".into());
        }
        if self.timeout <= 0.0 {
            return Err("llm.timeout must be > 0".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherSettings {
    /// e.g. "Mumbai". Empty = ask every time.
    #[serde(default)]
    pub default_location: String,

    #[serde(default = "default_8_0")]
    pub timeout: f64,
}

fn default_8_0() -> f64 {
    8.0
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            default_location: String::new(),
            timeout: default_8_0(),
        }
    }
}

impl WeatherSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.timeout <= 0.0 {
            return Err("tools.weather.timeout must be > 0".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSettings {
    /// "in-en" for India
    #[serde(default = "default_region")]
    pub region: String,

    /// "duckduckgo" = force one engine only
    #[serde(default = "default_auto")]
    pub backend: String,

    #[serde(default = "default_5")]
    pub max_results: u32,

    #[serde(default = "default_8")]
    pub timeout: u32,
}

fn default_region() -> String {
    "us-en".into()
}
fn default_auto() -> String {
    "auto".into()
}
fn default_5() -> u32 {
    5
}
fn default_8() -> u32 {
    8
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            region: default_region(),
            backend: default_auto(),
            max_results: default_5(),
            timeout: default_8(),
        }
    }
}

impl SearchSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=30).contains(&self.max_results) {
            return Err("tools.search.max_results must be in [1, 30]".into());
        }
        if self.timeout < 1 {
            return Err("tools.search.timeout must be >= 1".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemindersSettings {
    #[serde(default = "default_reminders_path")]
    pub path: PathBuf,
}

fn default_reminders_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".assistant")
        .join("reminders.json")
}

impl Default for RemindersSettings {
    fn default() -> Self {
        Self {
            path: default_reminders_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsSettings {
    /// Weather and web search contact third-party servers
    #[serde(default = "default_true")]
    pub online: bool,

    #[serde(default)]
    pub weather: WeatherSettings,

    #[serde(default)]
    pub search: SearchSettings,

    #[serde(default)]
    pub reminders: RemindersSettings,
}

fn default_true() -> bool {
    true
}

impl Default for ToolsSettings {
    fn default() -> Self {
        Self {
            online: true,
            weather: WeatherSettings::default(),
            search: SearchSettings::default(),
            reminders: RemindersSettings::default(),
        }
    }
}

impl ToolsSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.weather.validate()?;
        self.search.validate()?;
        Ok(())
    }
}

// Top-level Settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub audio: AudioSettings,

    #[serde(default)]
    pub vad: VADSettings,

    #[serde(default)]
    pub wake: WakeSettings,

    #[serde(default)]
    pub stt: STTSettings,

    #[serde(default)]
    pub tts: TTSSettings,

    #[serde(default)]
    pub llm: LLMSettings,

    #[serde(default)]
    pub tools: ToolsSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            vad: VADSettings::default(),
            wake: WakeSettings::default(),
            stt: STTSettings::default(),
            tts: TTSSettings::default(),
            llm: LLMSettings::default(),
            tools: ToolsSettings::default(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        self.vad.validate()?;
        self.wake.validate()?;
        self.llm.validate()?;
        self.tools.validate()?;
        Ok(())
    }
}

/// Defaults, overridden by a YAML file.
/// With no path, uses `./config.yaml` if it exists.
pub fn load_settings(path: Option<&Path>) -> Result<Settings, ConfigError> {
    let file = match path {
        Some(p) => p.to_path_buf(),
        None => {
            let default = PathBuf::from("config.yaml");
            if !default.exists() {
                return Ok(Settings::default());
            }
            default
        }
    };

    if !file.exists() {
        return Err(ConfigError::NotFound(file));
    }

    let text = std::fs::read_to_string(&file).map_err(|e| {
        ConfigError::Invalid(file.clone(), e.to_string())
    })?;

    let settings: Settings = serde_yaml::from_str(&text).map_err(|e| {
        ConfigError::Invalid(file.clone(), e.to_string())
    })?;

    settings.validate().map_err(|msg| {
        ConfigError::Invalid(file, msg)
    })?;

    Ok(settings)
}
