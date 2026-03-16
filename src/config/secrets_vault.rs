use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecretScope {
    Global,
    PerAgent(String),
    PerRequest,
}

#[derive(Debug, Clone)]
pub struct SecretEntry {
    pub key: String,
    pub value: String,
    pub provider: String,
    pub scope: SecretScope,
    pub created_at: SystemTime,
    pub rotated_at: Option<SystemTime>,
}

#[allow(dead_code)]
pub struct SecretsVault {
    secrets: DashMap<String, SecretEntry>,
}

#[allow(dead_code)]
impl SecretsVault {
    pub fn new() -> Self {
        Self {
            secrets: DashMap::new(),
        }
    }

    pub fn set(&self, key: &str, value: &str, provider: &str, scope: SecretScope) {
        self.secrets.insert(
            key.to_string(),
            SecretEntry {
                key: key.to_string(),
                value: value.to_string(),
                provider: provider.to_string(),
                scope,
                created_at: SystemTime::now(),
                rotated_at: None,
            },
        );
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(|e| e.value.clone())
    }

    pub fn rotate(&self, key: &str, new_value: &str) -> bool {
        if let Some(mut entry) = self.secrets.get_mut(key) {
            entry.value = new_value.to_string();
            entry.rotated_at = Some(SystemTime::now());
            true
        } else {
            false
        }
    }

    pub fn delete(&self, key: &str) -> bool {
        self.secrets.remove(key).is_some()
    }

    pub fn list_by_scope(&self, scope: &SecretScope) -> Vec<String> {
        self.secrets
            .iter()
            .filter(|e| match (&e.scope, scope) {
                (SecretScope::Global, SecretScope::Global) => true,
                (SecretScope::PerAgent(a), SecretScope::PerAgent(b)) => a == b,
                (SecretScope::PerRequest, SecretScope::PerRequest) => true,
                _ => false,
            })
            .map(|e| e.key.clone())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.secrets.len()
    }
}

impl Default for SecretsVault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let vault = SecretsVault::new();
        vault.set("api_key", "secret123", "openai", SecretScope::Global);
        assert_eq!(vault.get("api_key"), Some("secret123".to_string()));
    }

    #[test]
    fn test_get_missing_key_returns_none() {
        let vault = SecretsVault::new();
        assert_eq!(vault.get("nonexistent"), None);
    }

    #[test]
    fn test_rotate() {
        let vault = SecretsVault::new();
        vault.set("api_key", "old_secret", "openai", SecretScope::Global);
        let rotated = vault.rotate("api_key", "new_secret");
        assert!(rotated);
        assert_eq!(vault.get("api_key"), Some("new_secret".to_string()));
    }

    #[test]
    fn test_rotate_missing_key_returns_false() {
        let vault = SecretsVault::new();
        assert!(!vault.rotate("nonexistent", "value"));
    }

    #[test]
    fn test_rotate_sets_rotated_at() {
        let vault = SecretsVault::new();
        vault.set("api_key", "old", "openai", SecretScope::Global);
        // rotated_at should be None before rotation
        {
            let entry = vault.secrets.get("api_key").unwrap();
            assert!(entry.rotated_at.is_none());
        }
        vault.rotate("api_key", "new");
        // rotated_at should be Some after rotation
        let entry = vault.secrets.get("api_key").unwrap();
        assert!(entry.rotated_at.is_some());
    }

    #[test]
    fn test_delete() {
        let vault = SecretsVault::new();
        vault.set("api_key", "secret", "openai", SecretScope::Global);
        assert!(vault.delete("api_key"));
        assert_eq!(vault.get("api_key"), None);
    }

    #[test]
    fn test_delete_missing_key_returns_false() {
        let vault = SecretsVault::new();
        assert!(!vault.delete("nonexistent"));
    }

    #[test]
    fn test_count() {
        let vault = SecretsVault::new();
        assert_eq!(vault.count(), 0);
        vault.set("k1", "v1", "p1", SecretScope::Global);
        assert_eq!(vault.count(), 1);
        vault.set("k2", "v2", "p2", SecretScope::PerRequest);
        assert_eq!(vault.count(), 2);
        vault.delete("k1");
        assert_eq!(vault.count(), 1);
    }

    #[test]
    fn test_list_by_scope_global() {
        let vault = SecretsVault::new();
        vault.set("global1", "v1", "p1", SecretScope::Global);
        vault.set("global2", "v2", "p2", SecretScope::Global);
        vault.set(
            "agent1",
            "v3",
            "p3",
            SecretScope::PerAgent("bot".to_string()),
        );
        vault.set("req1", "v4", "p4", SecretScope::PerRequest);

        let mut keys = vault.list_by_scope(&SecretScope::Global);
        keys.sort();
        assert_eq!(keys, vec!["global1", "global2"]);
    }

    #[test]
    fn test_list_by_scope_per_agent() {
        let vault = SecretsVault::new();
        vault.set("k1", "v1", "p1", SecretScope::PerAgent("bot-a".to_string()));
        vault.set("k2", "v2", "p2", SecretScope::PerAgent("bot-b".to_string()));
        vault.set("k3", "v3", "p3", SecretScope::PerAgent("bot-a".to_string()));
        vault.set("k4", "v4", "p4", SecretScope::Global);

        let mut keys = vault.list_by_scope(&SecretScope::PerAgent("bot-a".to_string()));
        keys.sort();
        assert_eq!(keys, vec!["k1", "k3"]);

        let keys_b = vault.list_by_scope(&SecretScope::PerAgent("bot-b".to_string()));
        assert_eq!(keys_b, vec!["k2"]);
    }

    #[test]
    fn test_list_by_scope_per_request() {
        let vault = SecretsVault::new();
        vault.set("req1", "v1", "p1", SecretScope::PerRequest);
        vault.set("req2", "v2", "p2", SecretScope::PerRequest);
        vault.set("global1", "v3", "p3", SecretScope::Global);

        let mut keys = vault.list_by_scope(&SecretScope::PerRequest);
        keys.sort();
        assert_eq!(keys, vec!["req1", "req2"]);
    }

    #[test]
    fn test_list_by_scope_empty() {
        let vault = SecretsVault::new();
        vault.set("k1", "v1", "p1", SecretScope::Global);
        let keys = vault.list_by_scope(&SecretScope::PerRequest);
        assert!(keys.is_empty());
    }

    #[test]
    fn test_set_overwrites_existing() {
        let vault = SecretsVault::new();
        vault.set("key", "old", "p1", SecretScope::Global);
        vault.set("key", "new", "p2", SecretScope::Global);
        assert_eq!(vault.get("key"), Some("new".to_string()));
        assert_eq!(vault.count(), 1);
    }

    #[test]
    fn test_default_impl() {
        let vault = SecretsVault::default();
        assert_eq!(vault.count(), 0);
    }
}
