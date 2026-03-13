//! In-memory live node registry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::gateway::presence::now_ms;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSession {
    pub node_id: String,
    pub conn_id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub connected_at: u64,
}

/// A pending invoke waiting for a result from a node.
pub struct PendingInvoke {
    pub invoke_id: String,
    pub node_id: String,
    pub method: String,
    pub params: serde_json::Value,
    pub created_at: u64,
    pub tx: oneshot::Sender<serde_json::Value>,
}

pub struct NodeRegistry {
    nodes: HashMap<String, NodeSession>,
    pending_invokes: HashMap<String, PendingInvoke>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            pending_invokes: HashMap::new(),
        }
    }

    pub fn register(&mut self, session: NodeSession) {
        self.nodes.insert(session.node_id.clone(), session);
    }

    pub fn unregister(&mut self, node_id: &str) {
        self.nodes.remove(node_id);
    }

    pub fn list(&self) -> Vec<NodeSession> {
        self.nodes.values().cloned().collect()
    }

    pub fn get(&self, node_id: &str) -> Option<&NodeSession> {
        self.nodes.get(node_id)
    }

    pub fn rename(&mut self, node_id: &str, new_name: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.name = new_name.to_string();
            true
        } else {
            false
        }
    }

    /// Add a pending invoke, returns the receiver for the result.
    pub fn add_invoke(
        &mut self,
        invoke_id: String,
        node_id: String,
        method: String,
        params: serde_json::Value,
    ) -> oneshot::Receiver<serde_json::Value> {
        let (tx, rx) = oneshot::channel();
        self.pending_invokes.insert(
            invoke_id.clone(),
            PendingInvoke {
                invoke_id,
                node_id,
                method,
                params,
                created_at: now_ms(),
                tx,
            },
        );
        rx
    }

    /// Resolve a pending invoke with a result.
    pub fn resolve_invoke(&mut self, invoke_id: &str, result: serde_json::Value) -> bool {
        if let Some(pending) = self.pending_invokes.remove(invoke_id) {
            let _ = pending.tx.send(result);
            true
        } else {
            false
        }
    }

    /// Pull pending invokes for a given node.
    pub fn pending_for_node(&self, node_id: &str) -> Vec<(&str, &str, &serde_json::Value)> {
        self.pending_invokes
            .values()
            .filter(|p| p.node_id == node_id)
            .map(|p| (p.invoke_id.as_str(), p.method.as_str(), &p.params))
            .collect()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
