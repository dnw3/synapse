use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ServeConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
}

/// Authentication configuration for the web server.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AuthConfig {
    /// Whether authentication is enabled (default: false).
    #[serde(default)]
    pub enabled: bool,
    /// Password hash. If empty, first login sets the password.
    pub password_hash: Option<String>,
    /// JWT secret for session tokens. Auto-generated if not set.
    pub jwt_secret: Option<String>,
    /// Session duration in seconds (default: 86400 = 24h).
    #[serde(default = "default_session_duration")]
    pub session_duration: u64,
}

fn default_session_duration() -> u64 {
    86400
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            password_hash: None,
            jwt_secret: None,
            session_duration: default_session_duration(),
        }
    }
}

/// Multi-gateway deployment configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GatewayConfig {
    /// Unique identifier for this instance (defaults to hostname if unset).
    pub instance_id: Option<String>,
    /// URL for the shared store (e.g. "redis://...", "postgres://...").
    pub shared_store_url: Option<String>,
    /// Whether to enable leader election for singleton tasks like the scheduler.
    pub leader_election: Option<bool>,
}

/// A broadcast group — sends messages to multiple channels.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BroadcastGroup {
    /// Group name (e.g. "engineering").
    pub name: String,
    /// Targets: list of "platform:channel_id" strings.
    pub targets: Vec<String>,
}

/// ClawHub registry configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct HubConfig {
    /// ClawHub API base URL (default: https://hub.openclaw.ai/api).
    pub url: Option<String>,
    /// Environment variable name containing the API key.
    pub api_key_env: Option<String>,
}

/// A custom slash command defined in config.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct CustomCommand {
    /// Command name without the leading slash (e.g. "summarize").
    pub name: String,
    /// Description shown in /help.
    pub description: String,
    /// Prompt template. `{{input}}` is replaced with the command argument.
    pub prompt: String,
}
