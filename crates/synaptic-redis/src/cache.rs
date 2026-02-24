use async_trait::async_trait;
use redis::AsyncCommands;
use synaptic_core::{ChatResponse, SynapticError};

use crate::connection::{collect_matching_keys, RedisBackend, RedisConn};

/// Configuration for [`RedisCache`].
#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    /// Key prefix for all cache entries. Defaults to `"synaptic:cache:"`.
    pub prefix: String,
    /// Optional TTL in seconds. When set, cached entries expire automatically.
    pub ttl: Option<u64>,
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            prefix: "synaptic:cache:".to_string(),
            ttl: None,
        }
    }
}

/// Redis-backed implementation of the [`LlmCache`](synaptic_core::LlmCache) trait.
///
/// Stores serialized [`ChatResponse`] values under `{prefix}{key}` with
/// optional TTL expiration managed by Redis itself.
///
/// Supports both standalone Redis and Redis Cluster (with the `cluster` feature).
pub struct RedisCache {
    backend: RedisBackend,
    config: RedisCacheConfig,
}

impl RedisCache {
    /// Create a new `RedisCache` from a Redis URL with default configuration.
    pub fn from_url(url: &str) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config: RedisCacheConfig::default(),
        })
    }

    /// Create a new `RedisCache` from a Redis URL with custom configuration.
    pub fn from_url_with_config(
        url: &str,
        config: RedisCacheConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config,
        })
    }

    /// Create a new `RedisCache` from an existing [`RedisBackend`].
    #[allow(dead_code)]
    pub(crate) fn from_backend(backend: RedisBackend, config: RedisCacheConfig) -> Self {
        Self { backend, config }
    }

    /// Create a new `RedisCache` connecting to a Redis Cluster.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes(nodes: &[&str]) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config: RedisCacheConfig::default(),
        })
    }

    /// Create a new `RedisCache` connecting to a Redis Cluster with custom configuration.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes_with_config(
        nodes: &[&str],
        config: RedisCacheConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config,
        })
    }

    /// Build the full Redis key for a cache entry.
    fn redis_key(&self, key: &str) -> String {
        format!("{}{key}", self.config.prefix)
    }

    async fn get_connection(&self) -> Result<RedisConn, SynapticError> {
        self.backend.get_connection().await
    }
}

/// Helper to GET a key from Redis as an `Option<String>`.
async fn redis_get_string(con: &mut RedisConn, key: &str) -> Result<Option<String>, SynapticError> {
    let raw: Option<String> = con
        .get(key)
        .await
        .map_err(|e| SynapticError::Cache(format!("Redis GET error: {e}")))?;
    Ok(raw)
}

#[async_trait]
impl synaptic_core::LlmCache for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError> {
        let mut con = self.get_connection().await?;
        let redis_key = self.redis_key(key);

        let raw = redis_get_string(&mut con, &redis_key).await?;

        match raw {
            Some(json_str) => {
                let response: ChatResponse = serde_json::from_str(&json_str)
                    .map_err(|e| SynapticError::Cache(format!("JSON deserialize error: {e}")))?;
                Ok(Some(response))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError> {
        let mut con = self.get_connection().await?;
        let redis_key = self.redis_key(key);

        let json_str = serde_json::to_string(response)
            .map_err(|e| SynapticError::Cache(format!("JSON serialize error: {e}")))?;

        con.set::<_, _, ()>(&redis_key, &json_str)
            .await
            .map_err(|e| SynapticError::Cache(format!("Redis SET error: {e}")))?;

        // Apply TTL if configured
        if let Some(ttl_secs) = self.config.ttl {
            con.expire::<_, ()>(&redis_key, ttl_secs as i64)
                .await
                .map_err(|e| SynapticError::Cache(format!("Redis EXPIRE error: {e}")))?;
        }

        Ok(())
    }

    async fn clear(&self) -> Result<(), SynapticError> {
        let mut con = self.get_connection().await?;
        let pattern = format!("{}*", self.config.prefix);

        // Collect all matching keys (SCAN for standalone, KEYS for cluster)
        let keys = collect_matching_keys(&mut con, &pattern).await?;

        if !keys.is_empty() {
            // Delete in batches to avoid issues with large key sets
            for chunk in keys.chunks(100) {
                con.del::<_, ()>(chunk)
                    .await
                    .map_err(|e| SynapticError::Cache(format!("Redis DEL error: {e}")))?;
            }
        }

        Ok(())
    }
}
