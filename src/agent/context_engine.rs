use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Per-request context scope carrying identity and key/value variables.
#[derive(Debug, Clone)]
pub struct ContextScope {
    pub request_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub channel: Option<String>,
    pub variables: HashMap<String, Value>,
    pub parent_request_id: Option<String>,
    pub created_at: Instant,
}

/// Concurrent, TTL-based registry of per-request context scopes.
///
/// Scopes are created at request ingress (WebSocket, REST, bot adapter) and
/// torn down automatically when the TTL elapses. Child scopes inherit the
/// parent's variables at creation time, enabling sub-agent / multi-step
/// pipelines to share context without explicit threading.
pub struct ContextEngine {
    scopes: DashMap<String, ContextScope>,
    ttl: Duration,
}

impl ContextEngine {
    /// Create a new engine with the given time-to-live for each scope.
    pub fn new(ttl: Duration) -> Self {
        Self {
            scopes: DashMap::new(),
            ttl,
        }
    }

    /// Create a new root scope and insert it into the registry.
    pub fn create_scope(&self, request_id: &str, agent_id: &str, session_id: &str) -> ContextScope {
        let scope = ContextScope {
            request_id: request_id.to_string(),
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            channel: None,
            variables: HashMap::new(),
            parent_request_id: None,
            created_at: Instant::now(),
        };
        self.scopes.insert(request_id.to_string(), scope.clone());
        scope
    }

    /// Create a child scope derived from an existing parent.
    ///
    /// The child inherits the parent's `agent_id`, `session_id`, `channel`,
    /// and all variables at the moment of forking. Returns `None` if the
    /// parent scope is not found or has already expired.
    pub fn create_child_scope(&self, parent_id: &str, child_id: &str) -> Option<ContextScope> {
        let parent = self.scopes.get(parent_id)?;
        if parent.created_at.elapsed() > self.ttl {
            drop(parent);
            self.scopes.remove(parent_id);
            return None;
        }
        let mut child = parent.clone();
        child.request_id = child_id.to_string();
        child.parent_request_id = Some(parent_id.to_string());
        child.created_at = Instant::now();
        drop(parent);
        self.scopes.insert(child_id.to_string(), child.clone());
        Some(child)
    }

    /// Retrieve a scope by request ID.  Returns `None` if not found or expired.
    pub fn get(&self, request_id: &str) -> Option<ContextScope> {
        let entry = self.scopes.get(request_id)?;
        if entry.created_at.elapsed() > self.ttl {
            drop(entry);
            self.scopes.remove(request_id);
            return None;
        }
        Some(entry.clone())
    }

    /// Insert or update a variable on an existing scope.
    ///
    /// No-ops silently if the scope does not exist (already expired or was
    /// never created).
    pub fn set_variable(&self, request_id: &str, key: &str, value: Value) {
        if let Some(mut scope) = self.scopes.get_mut(request_id) {
            scope.variables.insert(key.to_string(), value);
        }
    }

    /// Remove all scopes whose TTL has elapsed.  Call periodically from a
    /// background task to prevent unbounded memory growth.
    pub fn cleanup_expired(&self) {
        self.scopes
            .retain(|_, scope| scope.created_at.elapsed() < self.ttl);
    }

    /// Number of currently tracked (possibly expired) scopes.
    pub fn active_count(&self) -> usize {
        self.scopes.len()
    }
}

/// Shared, reference-counted handle to a [`ContextEngine`].
pub type SharedContextEngine = Arc<ContextEngine>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn engine() -> ContextEngine {
        ContextEngine::new(Duration::from_secs(60))
    }

    #[test]
    fn test_create_scope() {
        let ce = engine();
        let scope = ce.create_scope("req-1", "agent-a", "sess-1");

        assert_eq!(scope.request_id, "req-1");
        assert_eq!(scope.agent_id, "agent-a");
        assert_eq!(scope.session_id, "sess-1");
        assert!(scope.channel.is_none());
        assert!(scope.parent_request_id.is_none());
        assert!(scope.variables.is_empty());
        assert_eq!(ce.active_count(), 1);

        // Should be retrievable.
        let fetched = ce.get("req-1").expect("scope must exist");
        assert_eq!(fetched.agent_id, "agent-a");
    }

    #[test]
    fn test_child_inherits_variables() {
        let ce = engine();
        ce.create_scope("parent", "agent-a", "sess-1");

        // Set a variable on the parent.
        ce.set_variable("parent", "user", serde_json::json!("alice"));
        ce.set_variable("parent", "lang", serde_json::json!("en"));

        let child = ce
            .create_child_scope("parent", "child")
            .expect("parent must exist");

        // Child inherits both variables.
        assert_eq!(
            child.variables.get("user"),
            Some(&serde_json::json!("alice"))
        );
        assert_eq!(child.variables.get("lang"), Some(&serde_json::json!("en")));

        // Child has correct linkage.
        assert_eq!(child.request_id, "child");
        assert_eq!(child.parent_request_id.as_deref(), Some("parent"));

        // Child inherits agent/session identity.
        assert_eq!(child.agent_id, "agent-a");
        assert_eq!(child.session_id, "sess-1");

        // Both scopes visible.
        assert_eq!(ce.active_count(), 2);
    }

    #[test]
    fn test_ttl_expiry() {
        // Very short TTL so we can test expiry without sleeping long.
        let ce = ContextEngine::new(Duration::from_millis(10));
        ce.create_scope("req-exp", "agent-b", "sess-2");

        // Scope is visible immediately.
        assert!(ce.get("req-exp").is_some());

        // Wait past the TTL.
        std::thread::sleep(Duration::from_millis(20));

        // Should now be treated as expired.
        assert!(ce.get("req-exp").is_none());

        // cleanup_expired should purge the entry.
        let ce2 = ContextEngine::new(Duration::from_millis(10));
        ce2.create_scope("req-gc", "agent-b", "sess-3");
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(ce2.active_count(), 1); // still in map before cleanup
        ce2.cleanup_expired();
        assert_eq!(ce2.active_count(), 0); // purged
    }

    #[test]
    fn test_set_variable() {
        let ce = engine();
        ce.create_scope("req-var", "agent-c", "sess-4");

        ce.set_variable("req-var", "count", serde_json::json!(42));
        ce.set_variable("req-var", "flag", serde_json::json!(true));

        // Overwrite an existing key.
        ce.set_variable("req-var", "count", serde_json::json!(99));

        let scope = ce.get("req-var").expect("scope must exist");
        assert_eq!(scope.variables["count"], serde_json::json!(99));
        assert_eq!(scope.variables["flag"], serde_json::json!(true));

        // set_variable on a non-existent scope is a no-op (no panic).
        ce.set_variable("does-not-exist", "k", serde_json::json!("v"));
    }
}
