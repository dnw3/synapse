use serde::Deserialize;

/// A model catalog entry defined via `[[models]]` in config.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ModelEntry {
    /// Canonical model name (e.g. "doubao-seed-2-0-pro-260215").
    pub name: String,
    /// Short aliases for quick switching (e.g. ["pro", "default"]).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Provider name (references a `[[providers]]` entry).
    pub provider: Option<String>,
    /// Override temperature for this model.
    pub temperature: Option<f64>,
    /// Override max_tokens for this model.
    pub max_tokens: Option<u32>,
    /// Default thinking level: off, low, medium, high.
    pub thinking: Option<String>,
}

/// A custom provider defined via `[[providers]]` in config.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ProviderEntry {
    /// Provider name (e.g. "ark").
    pub name: String,
    /// Base URL for the OpenAI-compatible API.
    pub base_url: String,
    /// Single API key env var (e.g. "ARK_API_KEY").
    pub api_key_env: Option<String>,
    /// Comma-separated multi-key env var for rotation (e.g. "ARK_API_KEYS").
    pub api_keys_env: Option<String>,
}

/// Channel-level model binding via `[[channel_models]]`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ChannelModelBinding {
    /// Channel identifier: "platform:channel_id" or "platform:*" for platform-wide.
    pub channel: String,
    /// Model name or alias to use for this channel.
    pub model: String,
}
