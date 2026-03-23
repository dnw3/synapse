mod common;
mod lark;

use serde::{Deserialize, Serialize};

/// Dynamic channel account configuration — one entry per [[channels.PLATFORM]] block.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChannelAccountConfig {
    /// Whether this account is enabled (default true).
    pub enabled: Option<bool>,
    /// Optional account identifier for multi-account setups.
    pub account_id: Option<String>,
    /// Platform-specific settings (deserialized lazily per platform).
    #[serde(flatten)]
    pub settings: serde_json::Value,
}

#[allow(unused_imports)]
pub use self::common::*;
pub use self::lark::*;

pub(crate) fn default_true() -> bool {
    true
}
pub(crate) fn default_4000() -> usize {
    4000
}
pub(crate) fn default_account_id() -> String {
    "default".to_string()
}

/// Resolve a secret value: try direct value first, then environment variable.
/// This allows dashboard UI to set values directly, while power users can
/// reference env vars in `synapse.toml`.
pub fn resolve_secret(
    direct: Option<&str>,
    env_name: Option<&str>,
    field_desc: &str,
) -> Result<String, String> {
    if let Some(val) = direct {
        if !val.is_empty() {
            return Ok(val.to_string());
        }
    }
    if let Some(env) = env_name {
        return std::env::var(env)
            .map_err(|_| format!("environment variable '{}' not set ({})", env, field_desc));
    }
    Err(format!("{} not configured", field_desc))
}

/// Access control allowlist for bot channels/users.
///
/// If both `allowed_users` and `allowed_channels` are empty (or unset),
/// the bot accepts all messages. Otherwise, only matching users/channels
/// are allowed.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct BotAllowlist {
    /// Allowed user IDs (platform-specific).
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Allowed channel/chat IDs.
    #[serde(default)]
    pub allowed_channels: Vec<String>,
}

#[allow(dead_code)]
impl BotAllowlist {
    /// Returns true if the allowlist is empty (no restrictions).
    pub fn is_empty(&self) -> bool {
        self.allowed_users.is_empty() && self.allowed_channels.is_empty()
    }

    /// Check if a user ID is allowed.
    pub fn is_user_allowed(&self, user_id: &str) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.iter().any(|u| u == user_id)
    }

    /// Check if a channel ID is allowed.
    pub fn is_channel_allowed(&self, channel_id: &str) -> bool {
        self.allowed_channels.is_empty() || self.allowed_channels.iter().any(|c| c == channel_id)
    }

    /// Check if a message from a user in a channel is allowed.
    /// Passes if either the user or channel is allowed.
    pub fn is_allowed(&self, user_id: Option<&str>, channel_id: Option<&str>) -> bool {
        if self.is_empty() {
            return true;
        }
        let user_ok = user_id.is_some_and(|u| self.is_user_allowed(u));
        let channel_ok = channel_id.is_some_and(|c| self.is_channel_allowed(c));
        user_ok || channel_ok
    }
}

/// Common interface for bot adapter configs used in the generic spawn loop.
#[allow(dead_code)]
pub trait AdapterConfig {
    fn enabled(&self) -> bool;
    fn account_id(&self) -> &str;
}

pub use crate::channels::dm::DmPolicy;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GroupPolicy {
    #[default]
    Allowlist,
    Open,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[allow(dead_code)]
pub struct GroupToolPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}
