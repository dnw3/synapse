use std::sync::Arc;

use async_trait::async_trait;
use colored::Colorize;
use serde_json::Value;
use synaptic::core::SynapticError;
use synaptic::middleware::{RiskLevel, SecurityConfirmationCallback};

/// Auto-approve callback for security middleware.
///
/// In Deep Agent mode we don't block for user confirmation but log the risk.
pub(crate) struct AutoApproveCallback;

#[async_trait]
impl SecurityConfirmationCallback for AutoApproveCallback {
    async fn confirm(
        &self,
        tool_name: &str,
        _args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        tracing::warn!(tool = %tool_name, risk = ?risk, "Tool auto-approved in agent mode");
        Ok(true)
    }
}

/// Interactive approval callback for CLI task mode.
///
/// Prompts the user in the terminal for High/Critical-risk tool calls.
/// Supports an "elevated" mode that temporarily auto-approves everything.
pub struct InteractiveApprovalCallback {
    elevated: Arc<std::sync::atomic::AtomicBool>,
}

impl InteractiveApprovalCallback {
    pub fn new() -> Self {
        Self {
            elevated: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Returns a handle to toggle elevated mode from outside (e.g. /elevated command).
    #[allow(dead_code)]
    pub fn elevated_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.elevated.clone()
    }
}

#[async_trait]
impl SecurityConfirmationCallback for InteractiveApprovalCallback {
    async fn confirm(
        &self,
        tool_name: &str,
        args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        if self.elevated.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!(tool = %tool_name, risk = ?risk, "Tool auto-approved (elevated mode)");
            return Ok(true);
        }

        if matches!(risk, RiskLevel::Low | RiskLevel::Medium) {
            return Ok(true);
        }

        let args_preview = {
            let s = args.to_string();
            if s.len() > 120 {
                format!("{}...", &s[..117])
            } else {
                s
            }
        };

        eprintln!();
        eprintln!(
            "{} {:?} risk tool call requires approval:",
            "approval:".red().bold(),
            risk
        );
        eprintln!("  Tool: {}", tool_name.cyan());
        eprintln!("  Args: {}", args_preview.dimmed());
        eprint!("  Allow? [y/N/a(llow all)] ");

        let answer = tokio::task::spawn_blocking(|| {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            input.trim().to_lowercase()
        })
        .await
        .unwrap_or_default();

        match answer.as_str() {
            "y" | "yes" => {
                eprintln!("  {}", "Approved.".green());
                Ok(true)
            }
            "a" | "all" | "allow all" => {
                self.elevated
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "  {} (elevated mode enabled for this session)",
                    "Approved all.".green()
                );
                Ok(true)
            }
            _ => {
                eprintln!("  {}", "Denied.".red());
                Ok(false)
            }
        }
    }
}

/// Safety callback for bot mode.
///
/// Blocks Critical-risk tools entirely, logs High-risk, auto-approves the rest.
pub struct BotSafetyCallback;

#[async_trait]
impl SecurityConfirmationCallback for BotSafetyCallback {
    async fn confirm(
        &self,
        tool_name: &str,
        _args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        match risk {
            RiskLevel::Critical => {
                tracing::error!(tool = %tool_name, "Tool blocked (Critical risk) in bot mode");
                Ok(false)
            }
            RiskLevel::High => {
                tracing::warn!(tool = %tool_name, "Tool has High risk — approved with logging in bot mode");
                Ok(true)
            }
            _ => Ok(true),
        }
    }
}
