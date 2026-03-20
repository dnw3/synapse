use serde::Deserialize;

use super::{default_account_id, default_true, BotAllowlist};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SlackBotConfig {
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DingTalkBotConfig {
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub api_url: String,
    pub phone_number: String,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// WeCom (WeChat Work / 企业微信) bot configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct WeChatBotConfig {
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub project_id: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}

/// WebChat bot configuration (embedded chat widget via HTTP API).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct WebChatBotConfig {
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub access_token: Option<String>,
    #[serde(default)]
    pub access_token_env: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub allowlist: BotAllowlist,
}
