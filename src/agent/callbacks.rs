use std::sync::Arc;

use async_trait::async_trait;
use colored::Colorize;
use serde_json::Value;
use synaptic::core::SynapticError;
use synaptic::middleware::{RiskLevel, SecurityConfirmationCallback};
use tokio::sync::mpsc;

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

/// An approval request sent from the agent to the WebSocket handler.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub args_preview: String,
    pub risk_level: String,
}

/// An approval response sent from the WebSocket client back to the agent.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApprovalResponse {
    pub approved: bool,
    /// If true, auto-approve all remaining calls in this session.
    #[serde(default)]
    pub allow_all: bool,
}

/// WebSocket-based approval callback for interactive security confirmation.
///
/// Sends approval requests to the client via a channel and waits for responses.
/// Times out after 30 seconds (defaults to deny).
#[allow(dead_code)]
pub struct WebSocketApprovalCallback {
    /// Channel to send approval requests to the WS handler.
    request_tx: mpsc::UnboundedSender<ApprovalRequest>,
    /// Channel to receive approval responses from the WS handler.
    response_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<ApprovalResponse>>,
    /// Session-level allow-all flag.
    allow_all: std::sync::atomic::AtomicBool,
}

impl WebSocketApprovalCallback {
    /// Create a new WebSocket approval callback with its associated channels.
    ///
    /// Returns the callback and a pair of (request_rx, response_tx) that the
    /// WS handler should use to relay requests/responses.
    #[allow(dead_code)]
    pub fn new() -> (
        Arc<Self>,
        mpsc::UnboundedReceiver<ApprovalRequest>,
        mpsc::UnboundedSender<ApprovalResponse>,
    ) {
        let (req_tx, req_rx) = mpsc::unbounded_channel();
        let (resp_tx, resp_rx) = mpsc::unbounded_channel();
        let callback = Arc::new(Self {
            request_tx: req_tx,
            response_rx: tokio::sync::Mutex::new(resp_rx),
            allow_all: std::sync::atomic::AtomicBool::new(false),
        });
        (callback, req_rx, resp_tx)
    }
}

#[async_trait]
impl SecurityConfirmationCallback for WebSocketApprovalCallback {
    async fn confirm(
        &self,
        tool_name: &str,
        args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        // Low/Medium risk: auto-approve
        if matches!(risk, RiskLevel::Low | RiskLevel::Medium) {
            return Ok(true);
        }

        // Session-level allow-all
        if self.allow_all.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(true);
        }

        let args_preview = {
            let s = args.to_string();
            if s.len() > 300 {
                format!("{}...", &s[..297])
            } else {
                s
            }
        };

        let request = ApprovalRequest {
            tool_name: tool_name.to_string(),
            args_preview,
            risk_level: format!("{:?}", risk),
        };

        // Send request to WS handler
        if self.request_tx.send(request).is_err() {
            // Channel closed — deny
            return Ok(false);
        }

        // Wait for response with 30s timeout
        let mut rx = self.response_rx.lock().await;
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
            Ok(Some(response)) => {
                if response.allow_all {
                    self.allow_all
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(response.approved)
            }
            _ => {
                // Timeout or channel closed — deny
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
