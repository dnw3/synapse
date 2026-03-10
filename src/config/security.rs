use std::collections::HashMap;

use serde::Deserialize;

use super::memory::default_true;

/// Rate limiting configuration for model API calls.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct RateLimitConfig {
    /// Burst size (max tokens in bucket). Default: 10.0.
    #[serde(default = "default_capacity")]
    pub capacity: f64,
    /// Tokens refilled per second. Default: 5.0.
    #[serde(default = "default_refill_rate")]
    pub refill_rate: f64,
}

fn default_capacity() -> f64 {
    10.0
}
fn default_refill_rate() -> f64 {
    5.0
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            capacity: default_capacity(),
            refill_rate: default_refill_rate(),
        }
    }
}

/// Secret masking configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SecretsConfig {
    /// Whether to mask the configured API key in output. Default: true.
    #[serde(default = "default_true")]
    pub mask_api_keys: bool,
    /// Additional environment variable names whose values should be masked.
    #[serde(default)]
    pub additional_env_vars: Vec<String>,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            mask_api_keys: true,
            additional_env_vars: Vec::new(),
        }
    }
}

/// Security middleware configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SecurityConfig {
    /// Whether security middleware is enabled. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Tool names that should be marked as High risk (require confirmation).
    #[serde(default)]
    pub high_risk_tools: Vec<String>,
    /// Tool names that should be blocked entirely (Critical risk).
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    /// Whether SSRF guard is enabled (blocks private IP access in tool args). Default: true.
    #[serde(default = "default_true")]
    pub ssrf_guard: bool,
    /// Number of consecutive tool failures before circuit breaker opens. Default: 5.
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: usize,
}

fn default_circuit_breaker_threshold() -> usize {
    5
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            high_risk_tools: Vec::new(),
            blocked_tools: Vec::new(),
            ssrf_guard: true,
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
        }
    }
}

/// Tool policy configuration — controls tool access via allow/deny lists,
/// owner-only restrictions, and named tool groups.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct ToolPolicyConfig {
    /// Tools restricted to the owner (e.g. `["execute", "write_file", "cron"]`).
    /// Supports `@group` references and glob patterns (`browser_*`).
    #[serde(default)]
    pub owner_only_tools: Vec<String>,
    /// Owner user IDs. If empty, owner-only enforcement is skipped (open access).
    #[serde(default)]
    pub owners: Vec<String>,
    /// Custom tool group definitions (e.g. `{ "@mygroup": ["tool_a", "tool_b"] }`).
    /// Custom groups override built-in groups of the same name.
    #[serde(default)]
    pub tool_groups: HashMap<String, Vec<String>>,
    /// Allow list — only these tools (after group expansion) are presented to the model.
    /// If empty, all tools are allowed (subject to deny list).
    #[serde(default)]
    pub tool_allow: Vec<String>,
    /// Deny list — these tools are removed from the model's tool list.
    #[serde(default)]
    pub tool_deny: Vec<String>,
}
