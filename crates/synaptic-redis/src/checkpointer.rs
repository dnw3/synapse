use async_trait::async_trait;
use redis::AsyncCommands;
use synaptic_core::SynapticError;
use synaptic_graph::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};

use crate::connection::{RedisBackend, RedisConn};

/// Configuration for the Redis-backed graph checkpointer.
#[derive(Debug, Clone)]
pub struct RedisCheckpointerConfig {
    /// Optional TTL in seconds for checkpoint keys. `None` means no expiry.
    pub ttl: Option<u64>,
    /// Key prefix. Defaults to `"synaptic"`.
    pub prefix: String,
}

impl Default for RedisCheckpointerConfig {
    fn default() -> Self {
        Self {
            ttl: None,
            prefix: "synaptic".to_string(),
        }
    }
}

impl RedisCheckpointerConfig {
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }
}

/// Redis-backed graph checkpointer.
///
/// Stores checkpoints as JSON values in Redis using the key scheme:
/// - Checkpoint data: `{prefix}:checkpoint:{thread_id}:{checkpoint_id}`
/// - Thread index (ordered list of checkpoint IDs): `{prefix}:idx:{thread_id}`
///
/// Supports both standalone Redis and Redis Cluster (with the `cluster` feature).
pub struct RedisCheckpointer {
    backend: RedisBackend,
    config: RedisCheckpointerConfig,
}

impl RedisCheckpointer {
    /// Create a new checkpointer from a Redis URL with default config.
    pub fn from_url(url: &str) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config: RedisCheckpointerConfig::default(),
        })
    }

    /// Create a new checkpointer from a Redis URL with custom config.
    pub fn from_url_with_config(
        url: &str,
        config: RedisCheckpointerConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::standalone(url)?,
            config,
        })
    }

    /// Create a new checkpointer from an existing [`RedisBackend`].
    #[allow(dead_code)]
    pub(crate) fn from_backend(backend: RedisBackend, config: RedisCheckpointerConfig) -> Self {
        Self { backend, config }
    }

    /// Create a new checkpointer connecting to a Redis Cluster.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes(nodes: &[&str]) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config: RedisCheckpointerConfig::default(),
        })
    }

    /// Create a new checkpointer connecting to a Redis Cluster with custom config.
    #[cfg(feature = "cluster")]
    pub fn from_cluster_nodes_with_config(
        nodes: &[&str],
        config: RedisCheckpointerConfig,
    ) -> Result<Self, SynapticError> {
        Ok(Self {
            backend: RedisBackend::cluster(nodes)?,
            config,
        })
    }

    fn checkpoint_key(&self, thread_id: &str, checkpoint_id: &str) -> String {
        format!(
            "{}:checkpoint:{}:{}",
            self.config.prefix, thread_id, checkpoint_id
        )
    }

    fn index_key(&self, thread_id: &str) -> String {
        format!("{}:idx:{}", self.config.prefix, thread_id)
    }

    async fn get_connection(&self) -> Result<RedisConn, SynapticError> {
        self.backend.get_connection().await
    }
}

#[async_trait]
impl Checkpointer for RedisCheckpointer {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError> {
        let mut conn = self.get_connection().await?;
        let data = serde_json::to_string(checkpoint)
            .map_err(|e| SynapticError::Store(format!("Serialize checkpoint: {e}")))?;

        let ck = self.checkpoint_key(&config.thread_id, &checkpoint.id);
        let idx = self.index_key(&config.thread_id);

        if let Some(ttl) = self.config.ttl {
            let _: () = conn
                .set_ex(&ck, &data, ttl)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis SET EX: {e}")))?;
        } else {
            let _: () = conn
                .set(&ck, &data)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis SET: {e}")))?;
        }

        // Append checkpoint ID to the ordered index for this thread
        let _: () = conn
            .rpush(&idx, &checkpoint.id)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis RPUSH: {e}")))?;

        if let Some(ttl) = self.config.ttl {
            let _: () = conn
                .expire(&idx, ttl as i64)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis EXPIRE idx: {e}")))?;
        }

        Ok(())
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError> {
        let mut conn = self.get_connection().await?;
        let idx = self.index_key(&config.thread_id);

        let checkpoint_id: Option<String> = if let Some(ref id) = config.checkpoint_id {
            Some(id.clone())
        } else {
            // Get the latest (last) checkpoint ID from the index
            conn.lrange::<_, Vec<String>>(&idx, -1, -1)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis LRANGE: {e}")))?
                .into_iter()
                .next()
        };

        let id = match checkpoint_id {
            Some(id) => id,
            None => return Ok(None),
        };

        let ck = self.checkpoint_key(&config.thread_id, &id);
        let data: Option<String> = conn
            .get(&ck)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis GET: {e}")))?;

        match data {
            None => Ok(None),
            Some(json) => {
                let cp: Checkpoint = serde_json::from_str(&json)
                    .map_err(|e| SynapticError::Store(format!("Deserialize checkpoint: {e}")))?;
                Ok(Some(cp))
            }
        }
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
        let mut conn = self.get_connection().await?;
        let idx = self.index_key(&config.thread_id);

        let ids: Vec<String> = conn
            .lrange(&idx, 0, -1)
            .await
            .map_err(|e| SynapticError::Store(format!("Redis LRANGE: {e}")))?;

        let mut checkpoints = Vec::with_capacity(ids.len());
        for id in ids {
            let ck = self.checkpoint_key(&config.thread_id, &id);
            let data: Option<String> = conn
                .get(&ck)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis GET: {e}")))?;
            if let Some(json) = data {
                let cp: Checkpoint = serde_json::from_str(&json)
                    .map_err(|e| SynapticError::Store(format!("Deserialize checkpoint: {e}")))?;
                checkpoints.push(cp);
            }
        }

        Ok(checkpoints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let cfg = RedisCheckpointerConfig::default();
        assert_eq!(cfg.prefix, "synaptic");
        assert!(cfg.ttl.is_none());
    }

    #[test]
    fn config_builder() {
        let cfg = RedisCheckpointerConfig::default()
            .with_ttl(3600)
            .with_prefix("myapp");
        assert_eq!(cfg.ttl, Some(3600));
        assert_eq!(cfg.prefix, "myapp");
    }

    #[test]
    fn from_url_valid() {
        let cp = RedisCheckpointer::from_url("redis://127.0.0.1/");
        assert!(cp.is_ok());
    }

    #[test]
    fn from_url_invalid() {
        let cp = RedisCheckpointer::from_url("not-a-valid-url");
        assert!(cp.is_err());
    }
}
