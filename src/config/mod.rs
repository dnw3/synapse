mod agent;
pub(crate) mod bot;
pub(crate) mod memory;
mod misc;
mod models;
mod security;
mod server;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use synaptic::config::SynapticAgentConfig;
use synaptic::core::SynapticError;

use crate::heartbeat::HeartbeatConfig;

// Re-export all config types so external imports remain unchanged.
pub use self::agent::*;
pub use self::bot::*;
pub use self::memory::{ContextConfig, MemoryConfig, ReflectionSynapseConfig, SessionConfig};
pub use self::misc::*;
pub use self::models::*;
pub use self::security::*;
pub use self::server::*;

/// Synapse configuration — extends the framework config with product-specific sections.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SynapseConfig {
    #[serde(flatten)]
    pub base: SynapticAgentConfig,

    /// Fallback model names for automatic failover.
    pub fallback_models: Option<Vec<String>>,

    /// Model catalog — named models with aliases and per-model parameters.
    #[serde(rename = "models")]
    pub model_catalog: Option<Vec<ModelEntry>>,
    /// Custom provider definitions with base_url and API key config.
    #[serde(rename = "providers")]
    pub provider_catalog: Option<Vec<ProviderEntry>>,
    /// Channel-level model bindings.
    #[serde(rename = "channel_models")]
    pub channel_model_bindings: Option<Vec<ChannelModelBinding>>,

    /// Lark bot configuration.
    pub lark: Option<LarkBotConfig>,
    /// Slack bot configuration.
    pub slack: Option<SlackBotConfig>,
    /// Telegram bot configuration.
    pub telegram: Option<TelegramBotConfig>,
    /// Discord bot configuration.
    pub discord: Option<DiscordBotConfig>,
    /// DingTalk bot configuration.
    pub dingtalk: Option<DingTalkBotConfig>,
    /// Mattermost bot configuration.
    pub mattermost: Option<MattermostBotConfig>,
    /// Matrix bot configuration.
    pub matrix: Option<MatrixBotConfig>,
    /// WhatsApp bot configuration.
    pub whatsapp: Option<WhatsAppBotConfig>,
    /// Microsoft Teams bot configuration.
    pub teams: Option<TeamsBotConfig>,
    /// Signal bot configuration.
    pub signal: Option<SignalBotConfig>,
    /// WeCom (WeChat Work) bot configuration.
    pub wechat: Option<WeChatBotConfig>,
    /// iMessage bot configuration.
    pub imessage: Option<IMessageBotConfig>,
    /// LINE bot configuration.
    pub line: Option<LineBotConfig>,
    /// Google Chat bot configuration.
    pub googlechat: Option<GoogleChatBotConfig>,
    /// IRC bot configuration.
    pub irc: Option<IrcBotConfig>,
    /// WebChat bot configuration.
    pub webchat: Option<WebChatBotConfig>,
    /// Twitch bot configuration.
    pub twitch: Option<TwitchBotConfig>,
    /// Nostr bot configuration.
    pub nostr: Option<NostrBotConfig>,
    /// Nextcloud Talk bot configuration.
    pub nextcloud: Option<NextcloudBotConfig>,
    /// Synology Chat bot configuration.
    pub synology: Option<SynologyBotConfig>,
    /// Tlon (Urbit) bot configuration.
    pub tlon: Option<TlonBotConfig>,
    /// Zalo bot configuration.
    pub zalo: Option<ZaloBotConfig>,

    /// Web server configuration.
    pub serve: Option<ServeConfig>,
    /// Docker sandbox configuration.
    pub docker: Option<DockerConfig>,
    /// Authentication configuration (for web server).
    pub auth: Option<AuthConfig>,

    /// Scheduled jobs.
    #[serde(rename = "schedule")]
    pub schedules: Option<Vec<ScheduleEntry>>,
    /// Voice configuration.
    pub voice: Option<VoiceConfig>,

    /// Multi-agent routing.
    #[serde(rename = "agent_routes")]
    pub agent_routes: Option<Vec<AgentRouteConfig>>,

    /// Rate limiting for model calls.
    pub rate_limit: Option<RateLimitConfig>,
    /// Secret masking configuration.
    pub secrets: Option<SecretsConfig>,
    /// Security middleware configuration.
    pub security: Option<SecurityConfig>,

    /// Custom slash commands.
    #[serde(rename = "command")]
    pub commands: Option<Vec<CustomCommand>>,
    /// Broadcast groups for multi-channel messaging.
    #[serde(rename = "broadcast_group")]
    pub broadcast_groups: Option<Vec<BroadcastGroup>>,
    /// Multi-gateway deployment configuration.
    pub gateway: Option<GatewayConfig>,
    /// ClawHub registry configuration.
    pub hub: Option<HubConfig>,

    /// Memory and context management configuration.
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Context injection configuration.
    #[serde(default)]
    pub context: ContextConfig,
    /// Session management configuration.
    #[serde(default)]
    pub session: SessionConfig,
    /// Sub-agent configuration.
    #[serde(default)]
    pub subagent: SubAgentConfig,
    /// Per-skill overrides.
    #[serde(default)]
    pub skill_overrides: HashMap<String, SkillOverrideConfig>,

    /// Skills system configuration (limits, bundled, extra dirs).
    #[serde(default)]
    pub skills: SkillsConfig,

    /// Heartbeat configuration for periodic proactive agent runs.
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,

    /// Tool policy configuration (allow/deny lists, owner-only, tool groups).
    #[serde(default)]
    pub tool_policy: ToolPolicyConfig,

    /// Post-session reflection for agent self-evolution.
    #[serde(default)]
    pub reflection: ReflectionSynapseConfig,

    /// Logging configuration (console, file, in-memory buffer).
    #[serde(default)]
    pub logging: crate::logging::LogConfig,

    /// Workspace directory for context files (SOUL.md, IDENTITY.md, etc.).
    /// Defaults to `~/.synapse/workspace/`.
    pub workspace: Option<String>,
}

impl SynapseConfig {
    /// Load configuration from a file (TOML, JSON, or YAML).
    ///
    /// Search order:
    /// 1. Explicit `path` (if provided)
    /// 2. `./synapse.{toml,json,yaml,yml}`
    /// 3. `~/.synapse/config.{toml,json,yaml,yml}`
    pub fn load(path: Option<&Path>) -> Result<Self, SynapticError> {
        synaptic::config::discover_and_load_named(path, "synapse")
    }

    /// Resolve the workspace directory path for the default agent.
    ///
    /// Priority: config `workspace` field → `~/.synapse/workspace/`.
    /// Creates the directory if it doesn't exist.
    pub fn workspace_dir(&self) -> PathBuf {
        self.workspace_dir_for_agent(None)
    }

    /// Resolve the workspace directory for a specific agent.
    ///
    /// - Default/unnamed agent: global `workspace` config → `~/.synapse/workspace/`
    /// - Named agent with explicit `workspace` in route config → that path
    /// - Named agent without explicit path → `~/.synapse/workspace-{name}/`
    pub fn workspace_dir_for_agent(&self, agent_name: Option<&str>) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        let dir = match agent_name {
            Some(name)
                if !name.is_empty()
                    && name != "default"
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
            {
                // Check if agent route has an explicit workspace path
                let agent_ws = self
                    .agent_routes
                    .as_ref()
                    .and_then(|routes| routes.iter().find(|r| r.name == name))
                    .and_then(|r| r.workspace.as_ref());

                if let Some(w) = agent_ws {
                    Self::expand_path(w, &home)
                } else {
                    // Default per-agent workspace: ~/.synapse/workspace-{name}/
                    home.join(".synapse").join(format!("workspace-{}", name))
                }
            }
            _ => {
                // Default agent: use global workspace config
                if let Some(ref w) = self.workspace {
                    Self::expand_path(w, &home)
                } else {
                    home.join(".synapse").join("workspace")
                }
            }
        };

        // Ensure directory exists
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }

        dir
    }

    /// Expand a path, resolving `~` or `~/` to home directory.
    fn expand_path(path: &str, home: &Path) -> PathBuf {
        if let Some(suffix) = path.strip_prefix("~/") {
            home.join(suffix)
        } else if path == "~" {
            home.to_path_buf()
        } else {
            PathBuf::from(path)
        }
    }

    /// Load config, falling back to sensible defaults if no config file exists.
    pub fn load_or_default(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        match Self::load(path) {
            Ok(config) => Ok(config),
            Err(SynapticError::Config(msg)) if path.is_none() => {
                eprintln!("note: no config file found ({}); using defaults", msg);
                Ok(Self::default())
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for SynapseConfig {
    fn default() -> Self {
        toml::from_str(
            r#"
[model]
provider = "openai"
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"

[agent]
system_prompt = "You are Synapse, a helpful AI assistant powered by the Synaptic framework. You can read and write files, execute commands, and help with complex coding tasks."
max_turns = 50

[agent.tools]
filesystem = true

[paths]
sessions_dir = ".sessions"
memory_file = "AGENTS.md"
"#,
        )
        .expect("default config must parse")
    }
}
