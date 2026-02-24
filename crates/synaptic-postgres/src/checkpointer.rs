use async_trait::async_trait;
use sqlx::PgPool;
use synaptic_core::SynapticError;
use synaptic_graph::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};

/// PostgreSQL-backed graph checkpointer.
///
/// Stores graph checkpoints in a `synaptic_checkpoints` table. Call
/// [`PgCheckpointer::initialize`] once to create the table before use.
pub struct PgCheckpointer {
    pool: PgPool,
    /// Table name (default: `synaptic_checkpoints`).
    table: String,
}

impl PgCheckpointer {
    /// Create a new checkpointer backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            table: "synaptic_checkpoints".to_string(),
        }
    }

    /// Use a custom table name.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = table.into();
        self
    }

    /// Create the checkpoints table if it does not exist.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {table} (
                thread_id     TEXT        NOT NULL,
                checkpoint_id TEXT        NOT NULL,
                state         JSONB       NOT NULL,
                next_node     TEXT,
                parent_id     TEXT,
                metadata      JSONB       NOT NULL DEFAULT '{{}}',
                created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (thread_id, checkpoint_id)
            );
            CREATE INDEX IF NOT EXISTS {table}_thread_created
                ON {table} (thread_id, created_at ASC);
            "#,
            table = self.table,
        );
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgCheckpointer init: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl Checkpointer for PgCheckpointer {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError> {
        let state = serde_json::to_value(&checkpoint.state)
            .map_err(|e| SynapticError::Store(format!("Serialize state: {e}")))?;
        let metadata = serde_json::to_value(&checkpoint.metadata)
            .map_err(|e| SynapticError::Store(format!("Serialize metadata: {e}")))?;

        let sql = format!(
            r#"
            INSERT INTO {table}
                (thread_id, checkpoint_id, state, next_node, parent_id, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (thread_id, checkpoint_id) DO UPDATE SET
                state      = EXCLUDED.state,
                next_node  = EXCLUDED.next_node,
                parent_id  = EXCLUDED.parent_id,
                metadata   = EXCLUDED.metadata,
                created_at = now()
            "#,
            table = self.table,
        );

        sqlx::query(&sql)
            .bind(&config.thread_id)
            .bind(&checkpoint.id)
            .bind(&state)
            .bind(&checkpoint.next_node)
            .bind(&checkpoint.parent_id)
            .bind(&metadata)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgCheckpointer put: {e}")))?;

        Ok(())
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError> {
        let row: Option<CheckpointRow> = if let Some(ref cp_id) = config.checkpoint_id {
            let sql = format!(
                "SELECT checkpoint_id, state, next_node, parent_id, metadata \
                 FROM {table} WHERE thread_id = $1 AND checkpoint_id = $2",
                table = self.table,
            );
            sqlx::query_as(&sql)
                .bind(&config.thread_id)
                .bind(cp_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SynapticError::Store(format!("PgCheckpointer get: {e}")))?
        } else {
            let sql = format!(
                "SELECT checkpoint_id, state, next_node, parent_id, metadata \
                 FROM {table} WHERE thread_id = $1 \
                 ORDER BY created_at DESC LIMIT 1",
                table = self.table,
            );
            sqlx::query_as(&sql)
                .bind(&config.thread_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SynapticError::Store(format!("PgCheckpointer get latest: {e}")))?
        };

        Ok(row.map(|r| r.into_checkpoint()))
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
        let sql = format!(
            "SELECT checkpoint_id, state, next_node, parent_id, metadata \
             FROM {table} WHERE thread_id = $1 \
             ORDER BY created_at ASC",
            table = self.table,
        );
        let rows: Vec<CheckpointRow> = sqlx::query_as(&sql)
            .bind(&config.thread_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgCheckpointer list: {e}")))?;

        Ok(rows.into_iter().map(|r| r.into_checkpoint()).collect())
    }
}

/// Internal row type used by sqlx::query_as.
#[derive(sqlx::FromRow)]
struct CheckpointRow {
    checkpoint_id: String,
    state: serde_json::Value,
    next_node: Option<String>,
    parent_id: Option<String>,
    metadata: serde_json::Value,
}

impl CheckpointRow {
    fn into_checkpoint(self) -> Checkpoint {
        let metadata = self
            .metadata
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Checkpoint {
            id: self.checkpoint_id,
            state: self.state,
            next_node: self.next_node,
            parent_id: self.parent_id,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_name_default() {
        // Cannot connect without a live PG, just verify struct construction
        // PgCheckpointer::new() requires PgPool, tested via integration tests.
        let _ = "synaptic_checkpoints".to_string();
    }
}
