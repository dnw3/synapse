use serde::Deserialize;

use super::memory::default_true;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DockerConfig {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<u32>,
    pub network: Option<bool>,
}

/// Voice mode configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct VoiceConfig {
    /// TTS provider: "openai", "elevenlabs".
    pub tts_provider: Option<String>,
    /// STT provider: "openai".
    pub stt_provider: Option<String>,
    /// Voice name for TTS.
    pub voice: Option<String>,
    /// API key env var for the voice provider.
    pub api_key_env: Option<String>,
    /// Wake word keyword (default: "synapse").
    pub wake_word: Option<String>,
    /// RMS amplitude threshold for silence detection (0.0–1.0, default: 0.02).
    pub silence_threshold: Option<f32>,
    /// Duration of continuous silence (in milliseconds) that ends a recording (default: 1500).
    pub silence_duration_ms: Option<u64>,
}

/// A scheduled job entry.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[allow(dead_code)]
pub struct ScheduleEntry {
    pub name: String,
    pub prompt: String,
    pub cron: Option<String>,
    pub interval_secs: Option<u64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub description: Option<String>,
}
