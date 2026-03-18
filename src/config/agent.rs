use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::bot::GroupSessionScope;
use super::memory::default_true;

// ---------------------------------------------------------------------------
// Multi-Agent: Agent definitions, Bindings, Broadcasts
// ---------------------------------------------------------------------------

/// Top-level multi-agent configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentsConfig {
    /// Default agent ID (fallback when no binding matches).
    #[serde(default = "default_agent_id")]
    pub default: String,
    /// Agent definition list.
    #[serde(default)]
    pub list: Vec<AgentDef>,
}

fn default_agent_id() -> String {
    "default".into()
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            default: default_agent_id(),
            list: Vec::new(),
        }
    }
}

/// Definition of an independent agent with its own workspace, model, tools, and session isolation.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentDef {
    /// Unique agent identifier (lowercase, [a-z0-9_-], max 64 chars).
    pub id: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Model override (resolved via ModelResolver).
    pub model: Option<String>,
    /// System prompt override.
    pub system_prompt: Option<String>,
    /// Workspace directory (default: `~/.synapse/agents/{id}/workspace`).
    pub workspace: Option<String>,
    /// DM session isolation level.
    #[serde(default)]
    pub dm_scope: DmSessionScope,
    /// Group session isolation level (overrides channel-level setting).
    pub group_session_scope: Option<GroupSessionScope>,
    /// Tool allowlist (empty = all tools allowed). Supports glob patterns.
    #[serde(default)]
    pub tool_allow: Vec<String>,
    /// Tool denylist. Supports glob patterns.
    #[serde(default)]
    pub tool_deny: Vec<String>,
    /// Per-agent skills directory.
    pub skills_dir: Option<String>,
}

/// DM session isolation level — controls how sessions are keyed for direct messages.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DmSessionScope {
    /// All DMs share a single session (unsafe for multi-user).
    Main,
    /// Each sender gets an independent session.
    PerPeer,
    /// Each channel + sender gets an independent session (recommended).
    #[default]
    PerChannelPeer,
    /// Each account + channel + sender gets an independent session.
    PerAccountChannelPeer,
}

/// Route binding — maps incoming messages to an agent based on channel/account/peer/guild/team.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Binding {
    /// Target agent ID.
    pub agent: String,
    /// Channel name constraint (e.g. "lark", "discord", "slack").
    pub channel: Option<String>,
    /// Account ID constraint (for multi-account channels).
    pub account_id: Option<String>,
    /// Peer match — bind to a specific DM or group.
    pub peer: Option<PeerMatch>,
    /// Discord guild ID constraint.
    pub guild_id: Option<String>,
    /// Slack team/workspace ID constraint.
    pub team_id: Option<String>,
    /// Discord role IDs (AND logic — user must have all listed roles).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Human-readable comment for this binding.
    pub comment: Option<String>,
}

/// Peer match — identifies a specific DM or group conversation.
#[derive(Debug, Clone, Deserialize)]
pub struct PeerMatch {
    /// Peer type.
    pub kind: PeerKind,
    /// Platform-specific peer ID.
    pub id: String,
}

/// Peer type for binding matches.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PeerKind {
    /// Direct message (1:1 DM).
    Direct,
    /// Group conversation.
    Group,
    /// Channel (e.g. Discord channel, Slack channel).
    Channel,
}

/// Agent broadcast group — fans out a single message to multiple agents.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentBroadcastGroup {
    /// Broadcast group name.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Channel constraint.
    pub channel: Option<String>,
    /// Peer ID constraint (exact match).
    pub peer_id: Option<String>,
    /// Agent IDs to fan out to.
    pub agents: Vec<String>,
    /// Execution strategy.
    #[serde(default)]
    pub strategy: BroadcastStrategy,
    /// Timeout in seconds for aggregated strategy (default: 60).
    #[serde(default = "default_broadcast_timeout")]
    pub timeout_secs: u64,
}

/// Broadcast execution strategy.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastStrategy {
    /// All agents process in parallel, each replies independently.
    #[default]
    Parallel,
    /// Agents process one after another.
    Sequential,
    /// All agents process in parallel, replies are merged into one message.
    Aggregated,
}

fn default_broadcast_timeout() -> u64 {
    60
}

// ---------------------------------------------------------------------------
// Agent directory helpers
// ---------------------------------------------------------------------------

/// Resolve the base directory for an agent: `~/.synapse/agents/{agent_id}/`.
#[allow(dead_code)]
pub fn agent_base_dir(agent_id: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".synapse").join("agents").join(agent_id)
}

/// Resolve the workspace directory for an agent definition.
/// Priority: explicit `workspace` field → `~/.synapse/agents/{id}/workspace/`.
pub fn agent_workspace_dir(agent_def: &AgentDef) -> PathBuf {
    if let Some(ref ws) = agent_def.workspace {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        if let Some(suffix) = ws.strip_prefix("~/") {
            home.join(suffix)
        } else {
            PathBuf::from(ws)
        }
    } else {
        agent_base_dir(&agent_def.id).join("workspace")
    }
}

/// Sessions directory for an agent: `~/.synapse/agents/{agent_id}/sessions/`.
#[allow(dead_code)]
pub fn agent_sessions_dir(agent_id: &str) -> PathBuf {
    agent_base_dir(agent_id).join("sessions")
}

/// Memory (LTM) directory for an agent: `~/.synapse/agents/{agent_id}/memory/`.
#[allow(dead_code)]
pub fn agent_memory_dir(agent_id: &str) -> PathBuf {
    agent_base_dir(agent_id).join("memory")
}

/// Ensure all standard agent directories exist.
#[allow(dead_code)]
pub fn ensure_agent_dirs(agent_id: &str) {
    let base = agent_base_dir(agent_id);
    std::fs::create_dir_all(base.join("workspace")).ok();
    std::fs::create_dir_all(base.join("sessions")).ok();
    std::fs::create_dir_all(base.join("memory")).ok();
}

// ---------------------------------------------------------------------------
// Sub-agent delegation configuration
// ---------------------------------------------------------------------------

/// Sub-agent delegation configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SubAgentConfig {
    /// Whether to enable sub-agent delegation (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum nested sub-agent depth (default: 3).
    #[serde(default = "default_subagent_depth")]
    pub max_depth: usize,
    /// Maximum concurrent sub-agents (default: 3).
    #[serde(default = "default_subagent_concurrent")]
    pub max_concurrent: usize,
    /// Maximum concurrent children per agent type (0 = unlimited, default: 0).
    #[serde(default)]
    pub max_children_per_agent: usize,
    /// Default timeout in seconds per sub-agent (default: 300).
    #[serde(default = "default_subagent_timeout")]
    pub timeout_secs: u64,
    /// Custom named agent type definitions.
    #[serde(default)]
    pub agents: Vec<SubAgentDefConfig>,
    /// Named tool profiles mapping profile names to tool lists.
    #[serde(default)]
    pub tool_profiles: HashMap<String, Vec<String>>,
}

/// TOML definition of a named sub-agent type.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SubAgentDefConfig {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    /// Model name alias (resolved via ModelResolver).
    pub model: Option<String>,
    /// Tool names to include (allowlist, supports glob patterns).
    #[serde(default)]
    pub tool_allow: Vec<String>,
    /// Tool names to exclude (denylist, supports glob patterns).
    #[serde(default)]
    pub tool_deny: Vec<String>,
    /// Timeout in seconds for this agent type.
    pub timeout_secs: Option<u64>,
    /// Maximum turns before stopping.
    pub max_turns: Option<usize>,
    /// Named tool profile to apply to this agent type.
    pub tool_profile: Option<String>,
    /// Permission mode: "default", "acceptEdits", "dontAsk", "bypassPermissions", "plan".
    pub permission_mode: Option<String>,
    /// Skill names to preload into the sub-agent's system prompt.
    #[serde(default)]
    pub skills: Vec<String>,
    /// If true, the task tool always runs this agent in background mode.
    #[serde(default)]
    pub background: bool,
}

fn default_subagent_depth() -> usize {
    3
}

fn default_subagent_concurrent() -> usize {
    3
}

fn default_subagent_timeout() -> u64 {
    300
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        let mut tool_profiles = HashMap::new();
        tool_profiles.insert(
            "read_only".to_string(),
            vec!["read_file", "grep", "glob", "list_dir", "Skill", "llm_task"]
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        );
        tool_profiles.insert(
            "coding".to_string(),
            vec![
                "read_file",
                "write_file",
                "edit_file",
                "execute",
                "grep",
                "glob",
                "list_dir",
                "Skill",
                "task",
                "TaskOutput",
                "llm_task",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        );
        tool_profiles.insert(
            "minimal".to_string(),
            vec!["read_file", "list_dir", "glob"]
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        );
        tool_profiles.insert(
            "messaging".to_string(),
            vec![
                "read_file",
                "grep",
                "glob",
                "list_dir",
                "Skill",
                "llm_task",
                "sessions_list",
                "sessions_history",
                "sessions_send",
                "sessions_spawn",
                "memory_search",
                "memory_get",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        );
        // "full" — empty list means no restrictions (all tools allowed)
        tool_profiles.insert("full".to_string(), Vec::new());
        Self {
            enabled: default_true(),
            max_depth: default_subagent_depth(),
            max_concurrent: default_subagent_concurrent(),
            max_children_per_agent: 0,
            timeout_secs: default_subagent_timeout(),
            agents: Vec::new(),
            tool_profiles,
        }
    }
}

/// Per-skill configuration override.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillOverrideConfig {
    /// Whether this skill is enabled (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Environment variable overrides for this skill.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Skills system configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SkillsConfig {
    /// Bundled skills allowlist (empty = all allowed).
    #[serde(default)]
    pub allow_bundled: Vec<String>,
    /// Additional skill directories to scan.
    #[serde(default)]
    pub extra_dirs: Vec<String>,
    /// Maximum skills to inject into system prompt (default: 150).
    #[serde(default = "default_max_skills_in_prompt")]
    pub max_skills_in_prompt: usize,
    /// Maximum characters for skill descriptions in prompt (default: 30000).
    #[serde(default = "default_max_skills_prompt_chars")]
    pub max_skills_prompt_chars: usize,
    /// Maximum skill file size in bytes (default: 256000).
    #[serde(default = "default_max_skill_file_bytes")]
    pub max_skill_file_bytes: usize,
    /// Maximum skill candidates to scan per directory (default: 300).
    #[serde(default = "default_max_candidates_per_dir")]
    pub max_candidates_per_dir: usize,
}

fn default_max_skills_in_prompt() -> usize {
    150
}

fn default_max_skills_prompt_chars() -> usize {
    30000
}

fn default_max_skill_file_bytes() -> usize {
    256000
}

fn default_max_candidates_per_dir() -> usize {
    300
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            allow_bundled: Vec::new(),
            extra_dirs: Vec::new(),
            max_skills_in_prompt: default_max_skills_in_prompt(),
            max_skills_prompt_chars: default_max_skills_prompt_chars(),
            max_skill_file_bytes: default_max_skill_file_bytes(),
            max_candidates_per_dir: default_max_candidates_per_dir(),
        }
    }
}

/// Agent routing rule — maps messages to specific agents by pattern or channel.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AgentRouteConfig {
    /// Agent name/identifier.
    pub name: String,
    /// Human-readable description of this route.
    pub description: Option<String>,
    /// Model override for this agent.
    pub model: Option<String>,
    /// System prompt override.
    pub system_prompt: Option<String>,
    /// Pattern to match against incoming messages (regex).
    pub pattern: Option<String>,
    /// Channel names this agent handles.
    #[serde(default)]
    pub channels: Vec<String>,
    /// User IDs this agent handles (empty = all users).
    #[serde(default)]
    pub users: Vec<String>,
    /// Manual priority override (higher wins). If unset, computed from specificity.
    pub priority: Option<u32>,
    /// Per-agent workspace directory override.
    /// If unset, non-default agents use `~/.synapse/workspace-{name}/`.
    pub workspace: Option<String>,
}
