use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::{params, Connection};
use synaptic_core::SynapticError;
use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};

/// SQLite-backed graph checkpointer.
///
/// Stores graph state checkpoints in a local SQLite database file (or in-memory
/// for testing). Uses `tokio::task::spawn_blocking` to avoid blocking the async
/// runtime during SQLite operations.
///
/// # Example
///
/// ```rust,no_run
/// use synaptic_sqlite::SqliteCheckpointer;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // File-based (persists across restarts)
/// let cp = SqliteCheckpointer::new("/var/lib/myapp/checkpoints.db")?;
///
/// // In-memory (for testing)
/// let cp = SqliteCheckpointer::in_memory()?;
/// # Ok(())
/// # }
/// ```
pub struct SqliteCheckpointer {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteCheckpointer {
    /// Create a new checkpointer backed by a SQLite database file.
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, SynapticError> {
        let conn = Connection::open(path)
            .map_err(|e| SynapticError::Store(format!("SQLite open: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS synaptic_checkpoints (
                thread_id     TEXT    NOT NULL,
                checkpoint_id TEXT    NOT NULL,
                state         TEXT    NOT NULL,
                created_at    INTEGER NOT NULL,
                PRIMARY KEY (thread_id, checkpoint_id)
            );
            CREATE TABLE IF NOT EXISTS synaptic_checkpoint_idx (
                thread_id     TEXT    NOT NULL,
                checkpoint_id TEXT    NOT NULL,
                seq           INTEGER NOT NULL,
                PRIMARY KEY (thread_id, checkpoint_id)
            );",
        )
        .map_err(|e| SynapticError::Store(format!("SQLite create tables: {e}")))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory checkpointer (useful for testing).
    pub fn in_memory() -> Result<Self, SynapticError> {
        Self::new(":memory:")
    }
}

#[async_trait]
impl Checkpointer for SqliteCheckpointer {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError> {
        let conn = Arc::clone(&self.conn);
        let thread_id = config.thread_id.clone();
        let checkpoint_id = checkpoint.id.clone();
        let data = serde_json::to_string(checkpoint)
            .map_err(|e| SynapticError::Store(format!("Serialize: {e}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("Lock: {e}")))?;

            conn.execute(
                "INSERT OR REPLACE INTO synaptic_checkpoints \
                 (thread_id, checkpoint_id, state, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![thread_id, checkpoint_id, data, now],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite INSERT: {e}")))?;

            // Determine next sequence number for this thread
            let max_seq: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(seq), -1) FROM synaptic_checkpoint_idx \
                     WHERE thread_id = ?1",
                    params![thread_id],
                    |row| row.get(0),
                )
                .unwrap_or(-1);

            conn.execute(
                "INSERT OR IGNORE INTO synaptic_checkpoint_idx \
                 (thread_id, checkpoint_id, seq) VALUES (?1, ?2, ?3)",
                params![thread_id, checkpoint_id, max_seq + 1],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite INSERT idx: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking: {e}")))?
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError> {
        let conn = Arc::clone(&self.conn);
        let thread_id = config.thread_id.clone();
        let checkpoint_id = config.checkpoint_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("Lock: {e}")))?;

            // Resolve checkpoint ID: explicit or latest by seq
            let resolved_id: Option<String> = if let Some(ref id) = checkpoint_id {
                Some(id.clone())
            } else {
                conn.query_row(
                    "SELECT checkpoint_id FROM synaptic_checkpoint_idx \
                     WHERE thread_id = ?1 ORDER BY seq DESC LIMIT 1",
                    params![thread_id],
                    |row| row.get(0),
                )
                .ok()
            };

            let id = match resolved_id {
                Some(id) => id,
                None => return Ok(None),
            };

            let data: Option<String> = conn
                .query_row(
                    "SELECT state FROM synaptic_checkpoints \
                     WHERE thread_id = ?1 AND checkpoint_id = ?2",
                    params![thread_id, id],
                    |row| row.get(0),
                )
                .ok();

            match data {
                None => Ok(None),
                Some(json) => {
                    let cp: Checkpoint = serde_json::from_str(&json)
                        .map_err(|e| SynapticError::Store(format!("Deserialize: {e}")))?;
                    Ok(Some(cp))
                }
            }
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking: {e}")))?
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
        let conn = Arc::clone(&self.conn);
        let thread_id = config.thread_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("Lock: {e}")))?;

            let mut stmt = conn
                .prepare(
                    "SELECT c.state \
                     FROM synaptic_checkpoints c \
                     JOIN synaptic_checkpoint_idx i \
                       ON c.thread_id = i.thread_id AND c.checkpoint_id = i.checkpoint_id \
                     WHERE c.thread_id = ?1 \
                     ORDER BY i.seq ASC",
                )
                .map_err(|e| SynapticError::Store(format!("SQLite prepare: {e}")))?;

            let checkpoints: Result<Vec<Checkpoint>, SynapticError> = stmt
                .query_map(params![thread_id], |row| row.get::<_, String>(0))
                .map_err(|e| SynapticError::Store(format!("SQLite query: {e}")))?
                .filter_map(|r| r.ok())
                .map(|json| {
                    serde_json::from_str(&json)
                        .map_err(|e| SynapticError::Store(format!("Deserialize: {e}")))
                })
                .collect();

            checkpoints
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking: {e}")))?
    }
}
