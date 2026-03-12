use serde::Deserialize;

fn default_true() -> bool {
    true
}
fn default_4000() -> usize {
    4000
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
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct BotAllowlist {
    /// Allowed user IDs (platform-specific).
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Allowed channel/chat IDs.
    #[serde(default)]
    pub allowed_channels: Vec<String>,
}

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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LarkRenderMode {
    #[default]
    Auto,
    Raw,
    Card,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupSessionScope {
    #[default]
    Group,
    GroupSender,
    GroupTopic,
    GroupTopicSender,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DmPolicy {
    #[default]
    Open,
    Allowlist,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GroupPolicy {
    #[default]
    Allowlist,
    Open,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GroupToolPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GroupConfig {
    #[serde(default)]
    pub tool_policy: GroupToolPolicy,
    #[serde(default)]
    pub session_scope: Option<GroupSessionScope>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LarkBotConfig {
    // Auth
    pub app_id: String,
    pub app_secret: Option<String>,
    #[serde(default)]
    pub app_secret_env: Option<String>,
    pub verification_token: Option<String>,
    pub encrypt_key: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,

    // Messaging
    #[serde(default)]
    pub render_mode: LarkRenderMode,
    #[serde(default = "default_true")]
    pub streaming: bool,
    #[serde(default = "default_4000")]
    pub text_chunk_limit: usize,

    // Policy
    #[serde(default)]
    pub dm_policy: DmPolicy,
    #[serde(default)]
    pub group_policy: GroupPolicy,
    #[serde(default = "default_true")]
    pub require_mention: bool,
    #[serde(default)]
    pub allowlist: BotAllowlist,

    // Sessions
    #[serde(default)]
    pub group_session_scope: GroupSessionScope,
    #[serde(default)]
    pub reply_in_thread: bool,

    // Features
    #[serde(default = "default_true")]
    pub typing_indicator: bool,

    // Groups
    #[serde(default)]
    pub groups: std::collections::HashMap<String, GroupConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SlackBotConfig {
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    pub app_token: Option<String>,
    #[serde(default)]
    pub app_token_env: Option<String>,
    pub signing_secret: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TelegramBotConfig {
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DiscordBotConfig {
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DingTalkBotConfig {
    pub app_key: String,
    pub app_secret: Option<String>,
    #[serde(default)]
    pub app_secret_env: Option<String>,
    pub callback_port: Option<u16>,
    pub robot_code: Option<String>,
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MattermostBotConfig {
    pub url: String,
    pub token: Option<String>,
    #[serde(default)]
    pub token_env: Option<String>,
    pub team_id: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MatrixBotConfig {
    pub homeserver_url: String,
    pub user_id: String,
    pub password: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// WhatsApp bot configuration (Baileys-compatible REST/WebSocket bridge).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct WhatsAppBotConfig {
    pub phone_number_id: Option<String>,
    pub bridge_url: Option<String>,
    pub access_token: Option<String>,
    pub api_key_env: Option<String>,
    pub verify_token: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Microsoft Teams bot configuration (Bot Framework webhook mode).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TeamsBotConfig {
    pub app_id: String,
    pub app_password: Option<String>,
    #[serde(default)]
    pub app_password_env: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Signal bot configuration (signal-cli REST API bridge).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SignalBotConfig {
    pub api_url: String,
    pub phone_number: String,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// WeCom (WeChat Work / 企业微信) bot configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct WeChatBotConfig {
    pub corp_id: Option<String>,
    pub webhook_key: Option<String>,
    #[serde(default)]
    pub webhook_key_env: Option<String>,
    pub token: Option<String>,
    pub encoding_aes_key: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// iMessage bot configuration (BlueBubbles REST API bridge).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct IMessageBotConfig {
    pub api_url: String,
    pub password: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// LINE bot configuration (LINE Messaging API webhook mode).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LineBotConfig {
    pub channel_secret: Option<String>,
    #[serde(default)]
    pub channel_secret_env: Option<String>,
    pub channel_token: Option<String>,
    #[serde(default)]
    pub channel_token_env: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Google Chat bot configuration (webhook mode).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GoogleChatBotConfig {
    pub project_id: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// WebChat bot configuration (embedded chat widget via HTTP API).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct WebChatBotConfig {
    /// Port for the HTTP server (default: 8090).
    pub port: Option<u16>,
    /// Allowed CORS origins (empty = allow all).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Chat widget title (default: "Synapse Chat").
    pub widget_title: Option<String>,
    /// Access control allowlist.
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// IRC bot configuration (raw TCP connection to IRC server).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct IrcBotConfig {
    pub server: String,
    pub port: Option<u16>,
    pub nick: String,
    pub password: Option<String>,
    pub password_env: Option<String>,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Twitch bot configuration (IRC-based, connects to irc.chat.twitch.tv).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TwitchBotConfig {
    pub nick: String,
    pub oauth_token: Option<String>,
    #[serde(default)]
    pub oauth_token_env: Option<String>,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Nostr bot configuration (NIP-01 WebSocket relay protocol).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct NostrBotConfig {
    pub private_key: Option<String>,
    #[serde(default)]
    pub private_key_env: Option<String>,
    #[serde(default)]
    pub relays: Vec<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Nextcloud Talk bot configuration (REST long-polling).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct NextcloudBotConfig {
    pub url: String,
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub rooms: Vec<String>,
    pub poll_interval_secs: Option<u64>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Synology Chat bot configuration (incoming/outgoing webhook).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SynologyBotConfig {
    /// Port for the incoming webhook HTTP server (default: 8091).
    pub port: Option<u16>,
    /// Optional outgoing webhook URL for sending replies.
    pub outgoing_webhook_url: Option<String>,
    /// Access control allowlist.
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Tlon (Urbit) bot configuration (HTTP SSE).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TlonBotConfig {
    pub url: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// Zalo bot configuration (Zalo OA webhook mode).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ZaloBotConfig {
    pub access_token: Option<String>,
    #[serde(default)]
    pub access_token_env: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}
