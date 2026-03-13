//! In-memory exec approval request manager.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::gateway::presence::now_ms;

/// How long an approval request lives before expiring (5 minutes).
const APPROVAL_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,
    AllowOnce,
    Deny,
    AllowSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestPayload {
    pub request_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub node_id: Option<String>,
    pub created_at: u64,
}

pub struct ApprovalRecord {
    pub payload: ApprovalRequestPayload,
    pub tx: Option<oneshot::Sender<ApprovalDecision>>,
}

pub struct ExecApprovalManager {
    pending: HashMap<String, ApprovalRecord>,
    /// Commands allowed for the current session (from AllowSession decisions).
    session_allows: Vec<String>,
}

impl ExecApprovalManager {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            session_allows: Vec::new(),
        }
    }

    /// Create a new approval request. Returns a receiver for the decision.
    pub fn create(
        &mut self,
        payload: ApprovalRequestPayload,
    ) -> oneshot::Receiver<ApprovalDecision> {
        let (tx, rx) = oneshot::channel();
        let id = payload.request_id.clone();
        self.pending.insert(
            id,
            ApprovalRecord {
                payload,
                tx: Some(tx),
            },
        );
        rx
    }

    /// Resolve a pending approval request with a decision.
    pub fn resolve(&mut self, request_id: &str, decision: ApprovalDecision) -> bool {
        if let Some(mut record) = self.pending.remove(request_id) {
            if decision == ApprovalDecision::AllowSession {
                self.session_allows.push(record.payload.command.clone());
            }
            if let Some(tx) = record.tx.take() {
                let _ = tx.send(decision);
            }
            true
        } else {
            false
        }
    }

    /// Expire old pending requests.
    pub fn expire(&mut self) {
        let now = now_ms();
        self.pending
            .retain(|_, r| now.saturating_sub(r.payload.created_at) < APPROVAL_TTL_MS);
    }

    /// Check if a command is allowed for the current session.
    pub fn is_session_allowed(&self, command: &str) -> bool {
        self.session_allows.iter().any(|c| c == command)
    }

    /// Get a snapshot of pending requests (for dashboard display).
    pub fn get_snapshot(&mut self) -> Vec<ApprovalRequestPayload> {
        self.expire();
        self.pending.values().map(|r| r.payload.clone()).collect()
    }
}

impl Default for ExecApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}
