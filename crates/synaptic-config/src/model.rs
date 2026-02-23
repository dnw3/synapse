use serde::Deserialize;

/// Model provider configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Provider name: "openai", "anthropic", "gemini", "ollama", or any OpenAI-compatible.
    pub provider: String,
    /// Model identifier (e.g., "gpt-4", "claude-sonnet-4-20250514").
    pub model: String,
    /// Environment variable name containing the API key.
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,
    /// Custom base URL for OpenAI-compatible providers.
    pub base_url: Option<String>,
    /// Maximum output tokens.
    pub max_tokens: Option<u32>,
    /// Sampling temperature.
    pub temperature: Option<f64>,
}

fn default_api_key_env() -> String {
    "OPENAI_API_KEY".to_string()
}
