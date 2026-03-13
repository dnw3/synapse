//! Role-based scope checking for RPC methods.
//!
//! Implements a DEFAULT-DENY policy: every method must match a known
//! category or the caller needs `operator.admin`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Connection role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full-access operator (human user or admin).
    Operator,
    /// Remote node (machine-to-machine).
    Node,
}

impl Default for Role {
    fn default() -> Self {
        Self::Operator
    }
}

/// Methods restricted to the Node role.
const NODE_ROLE_METHODS: &[&str] = &[
    "node.invoke.result",
    "node.heartbeat",
    "node.register",
    "node.capabilities",
];

/// Methods available to any authenticated connection.
const CHAT_METHODS: &[&str] = &[
    "agent",
    "chat.send",
    "chat.stop",
    "chat.history",
    "chat.abort",
    "chat.inject",
    "agent.wait",
    "poll",
    "connect",
    "health",
    "status",
    "ping",
];

/// Read-only operator methods.
const READ_METHODS: &[&str] = &[
    "conversations.list",
    "conversations.get",
    "messages.list",
    "sessions.list",
    "sessions.get",
    "sessions.usage",
    "sessions.usage.timeseries",
    "sessions.usage.logs",
    "logs.query",
    "logs.tail",
    "config.get",
    "config.schema",
    "config.validate",
    "agents.list",
    "skills.list",
    "skills.status",
    "skills.bins",
    "schedules.list",
    "channels.list",
    "channels.status",
    "cron.list",
    "cron.status",
    "cron.runs",
    "usage.status",
    "usage.cost",
    "models.list",
    "tools.catalog",
    "workspace.list",
    "workspace.get",
    "store.search",
    "store.list",
    "store.detail",
    "store.status",
    "debug.health",
    "agent.identity.get",
    "files.list",
];

/// Write operator methods.
const WRITE_METHODS: &[&str] = &[
    "conversations.create",
    "conversations.delete",
    "conversations.update",
    "messages.send",
    "sessions.create",
    "sessions.delete",
    "sessions.patch",
    "sessions.compact",
    "config.set",
    "config.reload",
    "agents.create",
    "agents.update",
    "agents.delete",
    "skills.create",
    "skills.update",
    "skills.delete",
    "skills.install",
    "schedules.create",
    "schedules.update",
    "schedules.delete",
    "channels.create",
    "channels.update",
    "channels.delete",
    "channels.logout",
    "cron.add",
    "cron.update",
    "cron.remove",
    "cron.run",
    "workspace.set",
    "workspace.create",
    "workspace.delete",
    "workspace.reset",
    "store.install",
    "debug.invoke",
    "files.upload",
    "files.delete",
];

/// Approval-related methods.
const APPROVAL_METHODS: &[&str] = &["approval.list", "approval.approve", "approval.deny"];

/// Pairing-related methods.
const PAIRING_METHODS: &[&str] = &[
    "pairing.start",
    "pairing.confirm",
    "pairing.cancel",
    "pairing.list",
];

/// Check whether the given role and scopes permit calling `method`.
///
/// Returns `Ok(())` if allowed, `Err(reason)` if denied.
pub fn check_scope(method: &str, role: Role, scopes: &HashSet<String>) -> Result<(), String> {
    // Node-only methods
    if NODE_ROLE_METHODS.contains(&method) {
        return if role == Role::Node {
            Ok(())
        } else {
            Err(format!("Method '{method}' requires Node role"))
        };
    }

    // Chat / general methods — any authenticated connection
    if CHAT_METHODS.contains(&method) {
        return Ok(());
    }

    // Read methods — require operator.read OR operator.write
    if READ_METHODS.contains(&method) {
        if scopes.contains("operator.read")
            || scopes.contains("operator.write")
            || scopes.contains("operator.admin")
        {
            return Ok(());
        }
        return Err(format!(
            "Method '{method}' requires scope operator.read or operator.write"
        ));
    }

    // Write methods — require operator.write
    if WRITE_METHODS.contains(&method) {
        if scopes.contains("operator.write") || scopes.contains("operator.admin") {
            return Ok(());
        }
        return Err(format!("Method '{method}' requires scope operator.write"));
    }

    // Approval methods
    if APPROVAL_METHODS.contains(&method) {
        if scopes.contains("operator.approvals") || scopes.contains("operator.admin") {
            return Ok(());
        }
        return Err(format!(
            "Method '{method}' requires scope operator.approvals"
        ));
    }

    // Pairing methods
    if PAIRING_METHODS.contains(&method) {
        if scopes.contains("operator.pairing") || scopes.contains("operator.admin") {
            return Ok(());
        }
        return Err(format!("Method '{method}' requires scope operator.pairing"));
    }

    // DEFAULT DENY — unknown methods require operator.admin
    if scopes.contains("operator.admin") {
        return Ok(());
    }

    Err(format!(
        "Method '{method}' requires scope operator.admin (default deny)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_methods_allowed_for_any_role() {
        let scopes = HashSet::new();
        assert!(check_scope("health", Role::Operator, &scopes).is_ok());
        assert!(check_scope("agent", Role::Node, &scopes).is_ok());
    }

    #[test]
    fn node_methods_require_node_role() {
        let scopes = HashSet::new();
        assert!(check_scope("node.invoke.result", Role::Node, &scopes).is_ok());
        assert!(check_scope("node.invoke.result", Role::Operator, &scopes).is_err());
    }

    #[test]
    fn read_methods_require_read_scope() {
        let empty = HashSet::new();
        assert!(check_scope("conversations.list", Role::Operator, &empty).is_err());

        let read = HashSet::from(["operator.read".to_string()]);
        assert!(check_scope("conversations.list", Role::Operator, &read).is_ok());
    }

    #[test]
    fn default_deny_unknown_methods() {
        let empty = HashSet::new();
        assert!(check_scope("some.unknown.method", Role::Operator, &empty).is_err());

        let admin = HashSet::from(["operator.admin".to_string()]);
        assert!(check_scope("some.unknown.method", Role::Operator, &admin).is_ok());
    }

    #[test]
    fn new_rpc_methods_have_correct_scopes() {
        let read = HashSet::from(["operator.read".to_string()]);
        let write = HashSet::from(["operator.write".to_string()]);

        // Read methods
        assert!(check_scope("sessions.list", Role::Operator, &read).is_ok());
        assert!(check_scope("sessions.get", Role::Operator, &read).is_ok());
        assert!(check_scope("sessions.usage", Role::Operator, &read).is_ok());
        assert!(check_scope("agents.list", Role::Operator, &read).is_ok());
        assert!(check_scope("skills.status", Role::Operator, &read).is_ok());
        assert!(check_scope("channels.status", Role::Operator, &read).is_ok());
        assert!(check_scope("config.get", Role::Operator, &read).is_ok());
        assert!(check_scope("cron.list", Role::Operator, &read).is_ok());
        assert!(check_scope("usage.status", Role::Operator, &read).is_ok());
        assert!(check_scope("logs.tail", Role::Operator, &read).is_ok());
        assert!(check_scope("models.list", Role::Operator, &read).is_ok());
        assert!(check_scope("tools.catalog", Role::Operator, &read).is_ok());
        assert!(check_scope("workspace.list", Role::Operator, &read).is_ok());
        assert!(check_scope("store.search", Role::Operator, &read).is_ok());
        assert!(check_scope("debug.health", Role::Operator, &read).is_ok());
        assert!(check_scope("agent.identity.get", Role::Operator, &read).is_ok());

        // Write methods
        assert!(check_scope("sessions.patch", Role::Operator, &write).is_ok());
        assert!(check_scope("sessions.delete", Role::Operator, &write).is_ok());
        assert!(check_scope("agents.create", Role::Operator, &write).is_ok());
        assert!(check_scope("skills.install", Role::Operator, &write).is_ok());
        assert!(check_scope("channels.logout", Role::Operator, &write).is_ok());
        assert!(check_scope("config.set", Role::Operator, &write).is_ok());
        assert!(check_scope("cron.add", Role::Operator, &write).is_ok());
        assert!(check_scope("workspace.set", Role::Operator, &write).is_ok());
        assert!(check_scope("store.install", Role::Operator, &write).is_ok());
        assert!(check_scope("debug.invoke", Role::Operator, &write).is_ok());

        // Write methods should NOT be accessible with read-only scope
        let empty = HashSet::new();
        assert!(check_scope("sessions.delete", Role::Operator, &empty).is_err());
        assert!(check_scope("config.set", Role::Operator, &empty).is_err());
    }
}
