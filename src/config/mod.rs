mod agent;
pub(crate) mod bot;
pub(crate) mod memory;
mod misc;
mod models;
pub mod reset_policy;
pub mod secrets_vault;
mod security;
mod server;
pub mod watcher;

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

    /// Lark bot configuration (multi-account).
    #[serde(default)]
    pub lark: Vec<LarkBotConfig>,
    /// Slack bot configuration (multi-account).
    #[serde(default)]
    pub slack: Vec<SlackBotConfig>,
    /// Telegram bot configuration (multi-account).
    #[serde(default)]
    pub telegram: Vec<TelegramBotConfig>,
    /// Discord bot configuration (multi-account).
    #[serde(default)]
    pub discord: Vec<DiscordBotConfig>,
    /// DingTalk bot configuration (multi-account).
    #[serde(default)]
    pub dingtalk: Vec<DingTalkBotConfig>,
    /// Mattermost bot configuration (multi-account).
    #[serde(default)]
    pub mattermost: Vec<MattermostBotConfig>,
    /// Matrix bot configuration (multi-account).
    #[serde(default)]
    pub matrix: Vec<MatrixBotConfig>,
    /// WhatsApp bot configuration (multi-account).
    #[serde(default)]
    pub whatsapp: Vec<WhatsAppBotConfig>,
    /// Microsoft Teams bot configuration (multi-account).
    #[serde(default)]
    pub teams: Vec<TeamsBotConfig>,
    /// Signal bot configuration (multi-account).
    #[serde(default)]
    pub signal: Vec<SignalBotConfig>,
    /// WeCom (WeChat Work) bot configuration (multi-account).
    #[serde(default)]
    pub wechat: Vec<WeChatBotConfig>,
    /// iMessage bot configuration (multi-account).
    #[serde(default)]
    pub imessage: Vec<IMessageBotConfig>,
    /// LINE bot configuration (multi-account).
    #[serde(default)]
    pub line: Vec<LineBotConfig>,
    /// Google Chat bot configuration (multi-account).
    #[serde(default)]
    pub googlechat: Vec<GoogleChatBotConfig>,
    /// IRC bot configuration (multi-account).
    #[serde(default)]
    pub irc: Vec<IrcBotConfig>,
    /// WebChat bot configuration (multi-account).
    #[serde(default)]
    pub webchat: Vec<WebChatBotConfig>,
    /// Twitch bot configuration (multi-account).
    #[serde(default)]
    pub twitch: Vec<TwitchBotConfig>,
    /// Nostr bot configuration (multi-account).
    #[serde(default)]
    pub nostr: Vec<NostrBotConfig>,
    /// Nextcloud Talk bot configuration (multi-account).
    #[serde(default)]
    pub nextcloud: Vec<NextcloudBotConfig>,
    /// Synology Chat bot configuration (multi-account).
    #[serde(default)]
    pub synology: Vec<SynologyBotConfig>,
    /// Tlon (Urbit) bot configuration (multi-account).
    #[serde(default)]
    pub tlon: Vec<TlonBotConfig>,
    /// Zalo bot configuration (multi-account).
    #[serde(default)]
    pub zalo: Vec<ZaloBotConfig>,

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

    /// Multi-agent definitions (new format).
    pub agents: Option<AgentsConfig>,
    /// Route bindings: map incoming messages to agents.
    #[serde(default)]
    pub bindings: Vec<Binding>,
    /// Agent broadcast groups: fan out messages to multiple agents.
    #[serde(default)]
    pub broadcasts: Vec<AgentBroadcastGroup>,

    /// Legacy multi-agent routing (deprecated — use `agents` + `bindings` instead).
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

    /// Session reset policy (daily boundary, idle timeout, or never).
    #[serde(default)]
    pub session_reset: crate::config::reset_policy::ResetConfig,

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

    /// Get the effective agents config, migrating legacy `[[agent_routes]]` if needed.
    ///
    /// Returns `AgentsConfig` with at least a default agent.
    pub fn effective_agents(&self) -> AgentsConfig {
        // New format takes priority
        if let Some(ref agents) = self.agents {
            return agents.clone();
        }

        // Migrate legacy [[agent_routes]]
        if let Some(ref routes) = self.agent_routes {
            if !routes.is_empty() {
                let mut agents_config = AgentsConfig {
                    default: "default".into(),
                    list: Vec::new(),
                };
                for route in routes {
                    agents_config.list.push(AgentDef {
                        id: route.name.clone(),
                        description: route.description.clone(),
                        model: route.model.clone(),
                        system_prompt: route.system_prompt.clone(),
                        workspace: route.workspace.clone(),
                        dm_scope: DmSessionScope::default(),
                        group_session_scope: None,
                        tool_allow: Vec::new(),
                        tool_deny: Vec::new(),
                        skills_dir: None,
                    });
                }
                return agents_config;
            }
        }

        // No agents configured — return empty with defaults
        AgentsConfig::default()
    }

    /// Get the effective bindings, migrating legacy `[[agent_routes]]` if needed.
    pub fn effective_bindings(&self) -> Vec<Binding> {
        if self.agents.is_some() || !self.bindings.is_empty() {
            return self.bindings.clone();
        }

        // Migrate legacy [[agent_routes]] channel constraints to bindings
        if let Some(ref routes) = self.agent_routes {
            let mut bindings = Vec::new();
            for route in routes {
                if route.channels.is_empty() {
                    // Catch-all route — no binding needed, handled by default
                    continue;
                }
                for ch in &route.channels {
                    bindings.push(Binding {
                        agent: route.name.clone(),
                        channel: Some(ch.clone()),
                        ..Default::default()
                    });
                }
            }
            return bindings;
        }

        Vec::new()
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

/// Summary of which config sections changed between two [`SynapseConfig`] loads.
///
/// Used by [`crate::gateway::state::AppState::reload_config`] to apply only
/// the fields that actually changed without a full restart.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ConfigDiff {
    pub agents_changed: bool,
    pub bindings_changed: bool,
    pub tools_changed: bool,
    pub memory_changed: bool,
    pub schedules_changed: bool,
    pub auth_changed: bool,
    pub serve_changed: bool,
    pub channels_changed: bool,
}

#[allow(dead_code)]
impl ConfigDiff {
    /// Returns `true` if any field indicates a change.
    pub fn any_changed(&self) -> bool {
        self.agents_changed
            || self.bindings_changed
            || self.tools_changed
            || self.memory_changed
            || self.schedules_changed
            || self.auth_changed
            || self.serve_changed
            || self.channels_changed
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
