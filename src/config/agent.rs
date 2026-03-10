use std::collections::HashMap;

use serde::Deserialize;

use super::memory::default_true;

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
            vec![
                "read_file", "grep", "glob", "list_dir", "Skill", "llm_task",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        );
        tool_profiles.insert(
            "coding".to_string(),
            vec![
                "read_file", "write_file", "edit_file", "execute", "grep",
                "glob", "list_dir", "Skill", "task", "TaskOutput", "llm_task",
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
                "read_file", "grep", "glob", "list_dir", "Skill", "llm_task",
                "sessions_list", "sessions_history", "sessions_send", "sessions_spawn",
                "memory_search", "memory_get",
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
}
