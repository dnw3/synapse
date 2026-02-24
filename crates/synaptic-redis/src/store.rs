use async_trait::async_trait;
use redis::AsyncCommands;
use serde_json::Value;
use synaptic_core::{Item, SynapticError};

use crate::connection::{collect_matching_keys, RedisBackend, RedisConn};

/// Configuration for [`RedisStore`].
#[derive(Debug, Clone)]
pub struct RedisStoreConfig {
    /// Key prefix for all store entries. Defaults to `"synaptic:store:"`.
    pub prefix: String,
}

impl Default for RedisStoreConfig {
    fn default() -> Self {
        Self {
            prefix: "synaptic:store:".to_string(),
        }
    }
}

/// Redis-backed implementation of the [`Store`](synaptic_core::Store) trait.
///
/// Keys are stored in the format `{prefix}{namespace_joined_by_colon}:{key}`.
/// A Redis SET at `{prefix}__namespaces__` tracks all known namespace paths
/// for efficient [`list_namespaces`](synaptic_core::Store::list_namespaces) queries.
///
/// Supports both standalone Redis and Redis Cluster (with the `cluster` feature).
pub struct RedisStore {
    backend: RedisBackend,
    config: RedisStoreConfig,
}

impl RedisStore {
    /// Create a new `RedisStore` from a Redis URL with default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid.
    pub fn from_url(url: &str) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config: RedisStoreConfig::default(),
        })
    }

    /// Create a new `RedisStore` from a Redis URL with custom configuration.
    pub fn from_url_with_config(
        url: &str,
        config: RedisStoreConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config,
        })
    }

    /// Create a new `RedisStore` from an existing [`RedisBackend`].
    #[allow(dead_code)]
    pub(crate) fn from_backend(backend: RedisBackend, config: RedisStoreConfig) -> Self {
        Self { backend, config }
    }

    /// Create a new `RedisStore` connecting to a Redis Cluster.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes(nodes: &[&str]) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config: RedisStoreConfig::default(),
        })
    }

    /// Create a new `RedisStore` connecting to a Redis Cluster with custom configuration.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes_with_config(
        nodes: &[&str],
        config: RedisStoreConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config,
        })
    }

    /// Build the Redis key for a given namespace and item key.
    fn redis_key(&self, namespace: &[&str], key: &str) -> String {
        let ns = namespace.join(":");
        if ns.is_empty() {
            format!("{}:{}", self.config.prefix.trim_end_matches(':'), key)
        } else {
            format!("{}{ns}:{key}", self.config.prefix)
        }
    }

    /// Build the Redis key for the namespace index SET.
    fn namespace_index_key(&self) -> String {
        format!("{}__namespaces__", self.config.prefix)
    }

    /// Build the SCAN/KEYS pattern for a given namespace.
    fn scan_pattern(&self, namespace: &[&str]) -> String {
        let ns = namespace.join(":");
        if ns.is_empty() {
            format!("{}*", self.config.prefix)
        } else {
            format!("{}{ns}:*", self.config.prefix)
        }
    }

    /// Encode namespace as a string for storage in the namespace index.
    fn encode_namespace(namespace: &[&str]) -> String {
        namespace.join(":")
    }

    async fn get_connection(&self) -> Result<RedisConn, SynapticError> {
        self.backend.get_connection().await
    }
}

fn now_iso() -> String {
    format!("{:?}", std::time::SystemTime::now())
}

/// Helper to GET a key from Redis as an `Option<String>`.
async fn redis_get_string(con: &mut RedisConn, key: &str) -> Result<Option<String>, SynapticError> {
    let raw: Option<String> = con
        .get(key)
        .await
        .map_err(|e| SynapticError::Store(format!("Redis GET error: {e}")))?;
    Ok(raw)
}

#[async_trait]
impl synaptic_core::Store for RedisStore {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError> {
        let mut con = self.get_connection().await?;
        let redis_key = self.redis_key(namespace, key);

        let raw = redis_get_string(&mut con, &redis_key).await?;

        match raw {
            Some(json_str) => {
                let item: Item = serde_json::from_str(&json_str)
                    .map_err(|e| SynapticError::Store(format!("JSON deserialize error: {e}")))?;
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    async fn search(
        &self,
        namespace: &[&str],
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Item>, SynapticError> {
        let mut con = self.get_connection().await?;
        let pattern = self.scan_pattern(namespace);
        let ns_index_key = self.namespace_index_key();

        // Collect all matching keys (SCAN for standalone, KEYS for cluster)
        let all_keys = collect_matching_keys(&mut con, &pattern).await?;

        // Filter out the namespace index key
        let keys: Vec<String> = all_keys
            .into_iter()
            .filter(|k| k != &ns_index_key)
            .collect();

        // Load items
        let mut items: Vec<Item> = Vec::new();
        for k in &keys {
            let raw = redis_get_string(&mut con, k).await?;
            if let Some(json_str) = raw {
                if let Ok(item) = serde_json::from_str::<Item>(&json_str) {
                    // Apply substring filter if query is provided
                    if let Some(q) = query {
                        if item.key.contains(q) || item.value.to_string().contains(q) {
                            items.push(item);
                        }
                    } else {
                        items.push(item);
                    }
                }
            }
            if items.len() >= limit {
                break;
            }
        }

        items.truncate(limit);
        Ok(items)
    }

    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError> {
        let mut con = self.get_connection().await?;
        let redis_key = self.redis_key(namespace, key);
        let ns_index_key = self.namespace_index_key();
        let ns_encoded = Self::encode_namespace(namespace);

        // Check for existing item to preserve created_at
        let existing = redis_get_string(&mut con, &redis_key).await?;

        let now = now_iso();
        let created_at = existing
            .as_ref()
            .and_then(|json_str| serde_json::from_str::<Item>(json_str).ok())
            .map(|item| item.created_at)
            .unwrap_or_else(|| now.clone());

        let item = Item {
            namespace: namespace.iter().map(|s| s.to_string()).collect(),
            key: key.to_string(),
            value,
            created_at,
            updated_at: now,
            score: None,
        };

        let json_str = serde_json::to_string(&item)
            .map_err(|e| SynapticError::Store(format!("JSON serialize error: {e}")))?;

        con.set::<_, _, ()>(&redis_key, &json_str)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis SET error: {e}")))?;

        // Track namespace in the index
        con.sadd::<_, _, ()>(&ns_index_key, &ns_encoded)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis SADD error: {e}")))?;

        Ok(())
    }

    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError> {
        let mut con = self.get_connection().await?;
        let redis_key = self.redis_key(namespace, key);

        con.del::<_, ()>(&redis_key)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis DEL error: {e}")))?;

        Ok(())
    }

    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError> {
        let mut con = self.get_connection().await?;
        let ns_index_key = self.namespace_index_key();

        let members: Vec<String> = con
            .smembers(&ns_index_key)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis SMEMBERS error: {e}")))?;

        let prefix_str = if prefix.is_empty() {
            String::new()
        } else {
            prefix.join(":")
        };

        let namespaces: Vec<Vec<String>> = members
            .into_iter()
            .filter(|ns| prefix.is_empty() || ns.starts_with(&prefix_str))
            .map(|ns| ns.split(':').map(String::from).collect())
            .collect();

        Ok(namespaces)
    }
}
