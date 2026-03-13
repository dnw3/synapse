//! Node pairing with JSON file persistence.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::gateway::presence::now_ms;

/// How long a pending pairing request lives before expiring (10 minutes).
const PENDING_TTL_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingNodeRequest {
    pub request_id: String,
    pub node_name: String,
    pub public_key: Option<String>,
    pub device_id: Option<String>,
    pub platform: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedNode {
    pub node_id: String,
    pub name: String,
    pub public_key: Option<String>,
    pub device_id: Option<String>,
    pub platform: Option<String>,
    pub paired_at: u64,
    pub token_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct PairingData {
    pending: Vec<PendingNodeRequest>,
    paired: Vec<PairedNode>,
}

pub struct PairingStore {
    data: PairingData,
    path: PathBuf,
}

impl PairingStore {
    pub fn new() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".synapse")
            .join("pairing");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("pairing.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { data, path }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    fn prune_pending(&mut self) {
        let now = now_ms();
        self.data
            .pending
            .retain(|r| now.saturating_sub(r.created_at) < PENDING_TTL_MS);
    }

    /// Submit a new pairing request. Returns the request_id.
    pub fn request(&mut self, req: PendingNodeRequest) -> String {
        self.prune_pending();
        let id = req.request_id.clone();
        self.data.pending.push(req);
        self.save();
        id
    }

    /// Approve a pending request, returning the paired node.
    pub fn approve(&mut self, request_id: &str) -> Option<PairedNode> {
        self.prune_pending();
        let idx = self
            .data
            .pending
            .iter()
            .position(|r| r.request_id == request_id)?;
        let req = self.data.pending.remove(idx);
        let node_id = uuid::Uuid::new_v4().to_string();
        let paired = PairedNode {
            node_id: node_id.clone(),
            name: req.node_name,
            public_key: req.public_key,
            device_id: req.device_id,
            platform: req.platform,
            paired_at: now_ms(),
            token_hash: None,
        };
        self.data.paired.push(paired.clone());
        self.save();
        Some(paired)
    }

    /// Reject a pending request.
    pub fn reject(&mut self, request_id: &str) -> bool {
        self.prune_pending();
        let before = self.data.pending.len();
        self.data.pending.retain(|r| r.request_id != request_id);
        let removed = self.data.pending.len() < before;
        if removed {
            self.save();
        }
        removed
    }

    /// Verify a node_id is paired.
    pub fn verify(&self, node_id: &str) -> bool {
        self.data.paired.iter().any(|n| n.node_id == node_id)
    }

    /// List pending requests.
    pub fn list_pending(&mut self) -> Vec<PendingNodeRequest> {
        self.prune_pending();
        self.data.pending.clone()
    }

    /// List paired nodes.
    pub fn list_paired(&self) -> Vec<PairedNode> {
        self.data.paired.clone()
    }

    /// Remove a paired node.
    pub fn remove_paired(&mut self, node_id: &str) -> bool {
        let before = self.data.paired.len();
        self.data.paired.retain(|n| n.node_id != node_id);
        let removed = self.data.paired.len() < before;
        if removed {
            self.save();
        }
        removed
    }

    /// Rename a paired node.
    pub fn rename(&mut self, node_id: &str, new_name: &str) -> bool {
        if let Some(node) = self.data.paired.iter_mut().find(|n| n.node_id == node_id) {
            node.name = new_name.to_string();
            self.save();
            true
        } else {
            false
        }
    }
}

impl Default for PairingStore {
    fn default() -> Self {
        Self::new()
    }
}
