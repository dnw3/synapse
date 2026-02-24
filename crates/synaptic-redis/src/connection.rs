//! Internal connection abstraction for standalone and cluster Redis backends.

use redis::aio::ConnectionLike;
use redis::{Cmd, Pipeline, RedisFuture, Value};
use synaptic_core::SynapticError;

// ---------------------------------------------------------------------------
// RedisConn — connection enum (implements ConnectionLike → gets AsyncCommands)
// ---------------------------------------------------------------------------

/// A connection to either a standalone Redis node or a Redis Cluster.
///
/// Implements [`ConnectionLike`] so that all [`AsyncCommands`](redis::AsyncCommands)
/// work transparently regardless of the backend.
pub(crate) enum RedisConn {
    Standalone(redis::aio::MultiplexedConnection),
    #[cfg(feature = "cluster")]
    Cluster(redis::cluster_async::ClusterConnection),
}

impl ConnectionLike for RedisConn {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value> {
        match self {
            Self::Standalone(c) => c.req_packed_command(cmd),
            #[cfg(feature = "cluster")]
            Self::Cluster(c) => c.req_packed_command(cmd),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        match self {
            Self::Standalone(c) => c.req_packed_commands(cmd, offset, count),
            #[cfg(feature = "cluster")]
            Self::Cluster(c) => c.req_packed_commands(cmd, offset, count),
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Self::Standalone(c) => c.get_db(),
            #[cfg(feature = "cluster")]
            Self::Cluster(c) => c.get_db(),
        }
    }
}

// ---------------------------------------------------------------------------
// RedisBackend — client enum, creates connections on demand
// ---------------------------------------------------------------------------

/// A Redis client backend that can be either a standalone client or a cluster client.
pub(crate) enum RedisBackend {
    Standalone(redis::Client),
    #[cfg(feature = "cluster")]
    Cluster(redis::cluster::ClusterClient),
}

impl RedisBackend {
    /// Create a standalone backend from a Redis URL.
    pub fn standalone(url: &str) -> Result<Self, SynapticError> {
        let client = redis::Client::open(url)
            .map_err(|e| SynapticError::Store(format!("failed to connect to Redis: {e}")))?;
        Ok(Self::Standalone(client))
    }

    /// Create a cluster backend from a list of initial node URLs.
    #[cfg(feature = "cluster")]
    pub fn cluster(nodes: &[&str]) -> Result<Self, SynapticError> {
        let client = redis::cluster::ClusterClient::new(
            nodes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
        .map_err(|e| SynapticError::Store(format!("failed to create Redis Cluster client: {e}")))?;
        Ok(Self::Cluster(client))
    }

    /// Obtain a connection from this backend.
    pub async fn get_connection(&self) -> Result<RedisConn, SynapticError> {
        match self {
            Self::Standalone(client) => {
                let conn = client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|e| SynapticError::Store(format!("Redis connection error: {e}")))?;
                Ok(RedisConn::Standalone(conn))
            }
            #[cfg(feature = "cluster")]
            Self::Cluster(client) => {
                let conn = client.get_async_connection().await.map_err(|e| {
                    SynapticError::Store(format!("Redis Cluster connection error: {e}"))
                })?;
                Ok(RedisConn::Cluster(conn))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared utility: enumerate keys matching a pattern
// ---------------------------------------------------------------------------

/// Collect all Redis keys matching `pattern`.
///
/// - **Standalone**: uses incremental `SCAN` to avoid blocking.
/// - **Cluster**: uses `KEYS` which redis-rs automatically scatters across
///   all cluster nodes and gathers the results.
pub(crate) async fn collect_matching_keys(
    conn: &mut RedisConn,
    pattern: &str,
) -> Result<Vec<String>, SynapticError> {
    match conn {
        RedisConn::Standalone(c) => {
            let mut keys: Vec<String> = Vec::new();
            let mut cursor: u64 = 0;
            loop {
                let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(pattern)
                    .arg("COUNT")
                    .arg(100)
                    .query_async(c)
                    .await
                    .map_err(|e| SynapticError::Store(format!("Redis SCAN error: {e}")))?;
                keys.extend(batch);
                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }
            Ok(keys)
        }
        #[cfg(feature = "cluster")]
        RedisConn::Cluster(c) => {
            use redis::AsyncCommands;
            let keys: Vec<String> = c
                .keys(pattern)
                .await
                .map_err(|e| SynapticError::Store(format!("Redis KEYS error: {e}")))?;
            Ok(keys)
        }
    }
}
