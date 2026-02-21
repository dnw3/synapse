use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::Connection;
use synaptic_core::{ChatResponse, SynapticError};

/// Configuration for [`SqliteCache`].
#[derive(Debug, Clone)]
pub struct SqliteCacheConfig {
    /// Path to the SQLite database file. Use `":memory:"` for an in-memory database.
    pub path: String,
    /// Optional TTL in seconds. When set, cached entries older than this are
    /// treated as expired and excluded from lookups.
    pub ttl: Option<u64>,
}

impl SqliteCacheConfig {
    /// Create a new configuration with a file path.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ttl: None,
        }
    }

    /// Create a configuration for an in-memory SQLite database.
    pub fn in_memory() -> Self {
        Self {
            path: ":memory:".to_string(),
            ttl: None,
        }
    }

    /// Set the TTL (time-to-live) in seconds for cached entries.
    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl = Some(seconds);
        self
    }
}

/// SQLite-backed implementation of the [`LlmCache`](synaptic_core::LlmCache) trait.
///
/// Stores serialized [`ChatResponse`] values in a SQLite table with optional
/// TTL expiration. Uses `tokio::task::spawn_blocking` to avoid blocking the
/// async runtime during SQLite operations.
pub struct SqliteCache {
    conn: Arc<Mutex<Connection>>,
    ttl: Option<u64>,
}

impl SqliteCache {
    /// Create a new `SqliteCache` from the given configuration.
    ///
    /// This opens (or creates) the SQLite database and initializes the cache
    /// table if it does not already exist.
    pub fn new(config: SqliteCacheConfig) -> Result<Self, SynapticError> {
        let conn = Connection::open(&config.path)
            .map_err(|e| SynapticError::Cache(format!("SQLite open error: {e}")))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS llm_cache (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            )",
            [],
        )
        .map_err(|e| SynapticError::Cache(format!("SQLite create table error: {e}")))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            ttl: config.ttl,
        })
    }
}

#[async_trait]
impl synaptic_core::LlmCache for SqliteCache {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError> {
        let conn = self.conn.clone();
        let ttl = self.ttl;
        let key = key.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Cache(format!("lock error: {e}")))?;

            let query = if ttl.is_some() {
                "SELECT value FROM llm_cache WHERE key = ?1 AND created_at + ?2 > unixepoch()"
            } else {
                "SELECT value FROM llm_cache WHERE key = ?1"
            };

            let mut stmt = conn
                .prepare(query)
                .map_err(|e| SynapticError::Cache(format!("SQLite prepare error: {e}")))?;

            let result = if let Some(ttl) = ttl {
                stmt.query_row(rusqlite::params![key, ttl as i64], |row| {
                    row.get::<_, String>(0)
                })
            } else {
                stmt.query_row(rusqlite::params![key], |row| row.get::<_, String>(0))
            };

            match result {
                Ok(json_str) => {
                    let response: ChatResponse = serde_json::from_str(&json_str).map_err(|e| {
                        SynapticError::Cache(format!("JSON deserialize error: {e}"))
                    })?;
                    Ok(Some(response))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(SynapticError::Cache(format!("SQLite query error: {e}"))),
            }
        })
        .await
        .map_err(|e| SynapticError::Cache(format!("spawn_blocking error: {e}")))?
    }

    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError> {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = serde_json::to_string(response)
            .map_err(|e| SynapticError::Cache(format!("JSON serialize error: {e}")))?;

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Cache(format!("lock error: {e}")))?;

            conn.execute(
                "INSERT OR REPLACE INTO llm_cache (key, value, created_at) VALUES (?1, ?2, unixepoch())",
                rusqlite::params![key, value],
            )
            .map_err(|e| SynapticError::Cache(format!("SQLite insert error: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::Cache(format!("spawn_blocking error: {e}")))?
    }

    async fn clear(&self) -> Result<(), SynapticError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Cache(format!("lock error: {e}")))?;

            conn.execute("DELETE FROM llm_cache", [])
                .map_err(|e| SynapticError::Cache(format!("SQLite delete error: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::Cache(format!("spawn_blocking error: {e}")))?
    }
}
