use serde::{Deserialize, Serialize};

use super::{
    default_4000, default_account_id, default_true, BotAllowlist, DmPolicy, GroupPolicy,
    GroupToolPolicy,
};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LarkRenderMode {
    #[default]
    Auto,
    Raw,
    Card,
}

/// Lark card appearance configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LarkCardConfig {
    /// Header template color: blue, wathet, turquoise, green, yellow,
    /// orange, red, carmine, violet, purple, indigo, grey, default.
    #[serde(default = "default_card_template")]
    pub template: String,

    /// Header title text. Empty = use bot name.
    #[serde(default)]
    pub header_title: String,

    /// Header icon token (standard_icon token string, e.g. "chat_outlined").
    #[serde(default)]
    pub header_icon: String,

    /// Show thumbs up/down feedback buttons. Default: true.
    #[serde(default = "default_true")]
    pub show_feedback: bool,

    /// Show timestamp in card footer. Default: false.
    #[serde(default)]
    pub show_timestamp: bool,

    /// Show token usage (input/output) in card footer. Default: false.
    #[serde(default)]
    pub show_usage: bool,

    /// Show response latency in card footer. Default: false.
    #[serde(default)]
    pub show_latency: bool,

    /// Show LogID for debugging in card footer. Default: false.
    #[serde(default)]
    pub show_logid: bool,
}

fn default_card_template() -> String {
    "blue".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupSessionScope {
    #[default]
    Group,
    GroupSender,
    GroupTopic,
    GroupTopicSender,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[allow(dead_code)]
pub struct GroupConfig {
    #[serde(default)]
    pub tool_policy: GroupToolPolicy,
    #[serde(default)]
    pub session_scope: Option<GroupSessionScope>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct LarkBotConfig {
    #[serde(default = "default_account_id")]
    pub account_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
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

    /// Card styling configuration.
    #[serde(default)]
    pub card: LarkCardConfig,

    // Policy
    #[serde(default)]
    pub dm_policy: DmPolicy,
    #[serde(default)]
    pub group_policy: GroupPolicy,
    #[serde(default = "default_true")]
    pub require_mention: bool,
    #[serde(default)]
    pub allowlist: BotAllowlist,

    /// Chat ID of the owner/admin for receiving pairing approval cards.
    #[serde(default)]
    pub owner_chat_id: Option<String>,
    /// Pairing code TTL in seconds (default: 3600 = 1 hour).
    #[serde(default)]
    pub pairing_ttl_secs: Option<u64>,

    // Sessions
    /// DM session isolation level.
    #[serde(default)]
    pub dm_session_scope: Option<crate::config::DmSessionScope>,
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
