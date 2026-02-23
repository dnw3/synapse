use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;

use crate::{AgentMiddleware, ToolCallRequest, ToolCaller};

/// Risk level for a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Assesses the risk level of a tool call.
#[async_trait]
pub trait SecurityAnalyzer: Send + Sync {
    async fn assess(&self, tool_name: &str, args: &Value) -> Result<RiskLevel, SynapticError>;
}

/// Determines whether a tool call at a given risk level requires confirmation.
#[async_trait]
pub trait ConfirmationPolicy: Send + Sync {
    async fn should_confirm(&self, tool_name: &str, risk: RiskLevel)
        -> Result<bool, SynapticError>;
}

/// Callback for obtaining user confirmation before executing a risky tool call.
#[async_trait]
pub trait SecurityConfirmationCallback: Send + Sync {
    async fn confirm(
        &self,
        tool_name: &str,
        args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError>;
}

/// Rule-based security analyzer that maps tool names and argument patterns to risk levels.
pub struct RuleBasedAnalyzer {
    tool_risks: HashMap<String, RiskLevel>,
    arg_patterns: Vec<ArgPattern>,
    default_risk: RiskLevel,
}

/// A pattern that elevates risk when matched in tool arguments.
struct ArgPattern {
    key: String,
    pattern: String,
    risk: RiskLevel,
}

impl RuleBasedAnalyzer {
    pub fn new() -> Self {
        Self {
            tool_risks: HashMap::new(),
            arg_patterns: Vec::new(),
            default_risk: RiskLevel::Low,
        }
    }

    /// Set the default risk level for unknown tools.
    pub fn with_default_risk(mut self, risk: RiskLevel) -> Self {
        self.default_risk = risk;
        self
    }

    /// Set the risk level for a specific tool.
    pub fn with_tool_risk(mut self, tool_name: impl Into<String>, risk: RiskLevel) -> Self {
        self.tool_risks.insert(tool_name.into(), risk);
        self
    }

    /// Add an argument pattern that elevates risk.
    /// If the argument value for `key` contains `pattern`, the risk is elevated to `risk`.
    pub fn with_arg_pattern(
        mut self,
        key: impl Into<String>,
        pattern: impl Into<String>,
        risk: RiskLevel,
    ) -> Self {
        self.arg_patterns.push(ArgPattern {
            key: key.into(),
            pattern: pattern.into(),
            risk,
        });
        self
    }
}

impl Default for RuleBasedAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecurityAnalyzer for RuleBasedAnalyzer {
    async fn assess(&self, tool_name: &str, args: &Value) -> Result<RiskLevel, SynapticError> {
        let mut risk = self
            .tool_risks
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_risk);

        // Check argument patterns - elevate risk if matched
        for pattern in &self.arg_patterns {
            if let Some(val) = args.get(&pattern.key) {
                let val_str = match val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if val_str.contains(&pattern.pattern) && pattern.risk > risk {
                    risk = pattern.risk;
                }
            }
        }

        Ok(risk)
    }
}

/// Confirms tool calls that meet or exceed a risk threshold.
pub struct ThresholdConfirmationPolicy {
    threshold: RiskLevel,
}

impl ThresholdConfirmationPolicy {
    pub fn new(threshold: RiskLevel) -> Self {
        Self { threshold }
    }
}

#[async_trait]
impl ConfirmationPolicy for ThresholdConfirmationPolicy {
    async fn should_confirm(
        &self,
        _tool_name: &str,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        Ok(risk >= self.threshold)
    }
}

/// Middleware that assesses tool call risk and optionally requires confirmation.
pub struct SecurityMiddleware {
    analyzer: Arc<dyn SecurityAnalyzer>,
    policy: Arc<dyn ConfirmationPolicy>,
    callback: Arc<dyn SecurityConfirmationCallback>,
    /// Tools that bypass security checks entirely.
    bypass: HashSet<String>,
}

impl SecurityMiddleware {
    pub fn new(
        analyzer: Arc<dyn SecurityAnalyzer>,
        policy: Arc<dyn ConfirmationPolicy>,
        callback: Arc<dyn SecurityConfirmationCallback>,
    ) -> Self {
        Self {
            analyzer,
            policy,
            callback,
            bypass: HashSet::new(),
        }
    }

    /// Add tools that should bypass security checks.
    pub fn with_bypass(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.bypass = tools.into_iter().map(|s| s.into()).collect();
        self
    }
}

#[async_trait]
impl AgentMiddleware for SecurityMiddleware {
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let tool_name = &request.call.name;

        // Check bypass list
        if self.bypass.contains(tool_name) {
            return next.call(request).await;
        }

        // Assess risk
        let risk = self
            .analyzer
            .assess(tool_name, &request.call.arguments)
            .await?;

        // Check if confirmation is needed
        let needs_confirm = self.policy.should_confirm(tool_name, risk).await?;

        if needs_confirm {
            let confirmed = self
                .callback
                .confirm(tool_name, &request.call.arguments, risk)
                .await?;
            if !confirmed {
                return Err(SynapticError::Tool(format!(
                    "tool call '{}' rejected by security policy (risk: {:?})",
                    tool_name, risk
                )));
            }
        }

        next.call(request).await
    }
}
