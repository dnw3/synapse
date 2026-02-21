use async_trait::async_trait;
use redis::AsyncCommands;
use synaptic_core::{ChatResponse, SynapticError};

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
pub struct RedisCache {
    client: redis::Client,
    config: RedisCacheConfig,
}

impl RedisCache {
    /// Create a new `RedisCache` with an existing Redis client and configuration.
    pub fn new(client: redis::Client, config: RedisCacheConfig) -> Self {
        Self { client, config }
    }

    /// Create a new `RedisCache` from a Redis URL with default configuration.
    pub fn from_url(url: &str) -> Result<Self, SynapticError> {
        let client = redis::Client::open(url)
            .map_err(|e| SynapticError::Cache(format!("failed to connect to Redis: {e}")))?;
        Ok(Self {
            client,
            config: RedisCacheConfig::default(),
        })
    }

    /// Create a new `RedisCache` from a Redis URL with custom configuration.
    pub fn from_url_with_config(
        url: &str,
        config: RedisCacheConfig,
    ) -> Result<Self, SynapticError> {
        let client = redis::Client::open(url)
            .map_err(|e| SynapticError::Cache(format!("failed to connect to Redis: {e}")))?;
        Ok(Self { client, config })
    }

    /// Build the full Redis key for a cache entry.
    fn redis_key(&self, key: &str) -> String {
        format!("{}{key}", self.config.prefix)
    }

    async fn get_connection(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, SynapticError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SynapticError::Cache(format!("Redis connection error: {e}")))
    }
}

/// Helper to GET a key from Redis as an `Option<String>`.
async fn redis_get_string(
    con: &mut redis::aio::MultiplexedConnection,
    key: &str,
) -> Result<Option<String>, SynapticError> {
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

        // Collect all matching keys via SCAN, then delete them
        let mut cursor: u64 = 0;
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut con)
                .await
                .map_err(|e| SynapticError::Cache(format!("Redis SCAN error: {e}")))?;

            if !keys.is_empty() {
                con.del::<_, ()>(&keys)
                    .await
                    .map_err(|e| SynapticError::Cache(format!("Redis DEL error: {e}")))?;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }
}
