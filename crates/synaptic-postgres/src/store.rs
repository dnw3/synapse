use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use synaptic_core::{encode_namespace, now_iso, validate_table_name, Item, SynapticError};

/// Configuration for [`PgStore`].
#[derive(Debug, Clone)]
pub struct PgStoreConfig {
    /// Name of the PostgreSQL table used for key-value storage.
    pub table_name: String,
}

impl PgStoreConfig {
    /// Create a new configuration with the given table name.
    ///
    /// The table name is validated during [`PgStore::initialize`] to prevent
    /// SQL injection — only alphanumeric ASCII characters, underscores, and
    /// dots (for schema-qualified names) are accepted.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

/// PostgreSQL-backed implementation of the [`Store`](synaptic_core::Store) trait.
///
/// Uses a single table with `(namespace, key)` as the composite primary key
/// and stores values as JSONB. Full-text search is supported through a
/// `tsvector` generated column indexed with GIN, with a LIKE fallback.
///
/// # Example
///
/// ```rust,no_run
/// use sqlx::postgres::PgPoolOptions;
/// use synaptic_postgres::{PgStore, PgStoreConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPoolOptions::new()
///     .max_connections(5)
///     .connect("postgres://user:pass@localhost/mydb")
///     .await?;
///
/// let config = PgStoreConfig::new("synaptic_store");
/// let store = PgStore::new(pool, config);
/// store.initialize().await?;
/// # Ok(())
/// # }
/// ```
pub struct PgStore {
    pool: PgPool,
    config: PgStoreConfig,
}

impl PgStore {
    /// Create a new `PgStore` from an existing connection pool and config.
    pub fn new(pool: PgPool, config: PgStoreConfig) -> Self {
        Self { pool, config }
    }

    /// Ensure the backing table and indexes exist.
    ///
    /// Creates the key-value table, a namespace index, and a `tsvector`
    /// generated column with a GIN index for full-text search. This is
    /// idempotent and safe to call on every application startup.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let create_table = format!(
            r#"CREATE TABLE IF NOT EXISTS {table} (
                namespace  TEXT NOT NULL,
                key        TEXT NOT NULL,
                value      JSONB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (namespace, key)
            )"#,
            table = self.config.table_name,
        );
        sqlx::query(&create_table)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("failed to create table: {e}")))?;

        let create_ns_idx = format!(
            "CREATE INDEX IF NOT EXISTS {table}_namespace ON {table} (namespace)",
            table = self.config.table_name,
        );
        sqlx::query(&create_ns_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("failed to create namespace index: {e}")))?;

        // Add a tsvector generated column for full-text search.
        // ALTER TABLE ... ADD COLUMN IF NOT EXISTS is idempotent.
        let add_tsv = format!(
            r#"ALTER TABLE {table} ADD COLUMN IF NOT EXISTS tsv tsvector
               GENERATED ALWAYS AS (to_tsvector('simple', key || ' ' || value::text)) STORED"#,
            table = self.config.table_name,
        );
        sqlx::query(&add_tsv)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("failed to add tsvector column: {e}")))?;

        let create_tsv_idx = format!(
            "CREATE INDEX IF NOT EXISTS {table}_tsv ON {table} USING GIN (tsv)",
            table = self.config.table_name,
        );
        sqlx::query(&create_tsv_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("failed to create tsvector index: {e}")))?;

        Ok(())
    }

    /// Return a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &PgStoreConfig {
        &self.config
    }
}

#[async_trait]
impl synaptic_core::Store for PgStore {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError> {
        validate_table_name(&self.config.table_name)?;
        let ns = encode_namespace(namespace);

        let sql = format!(
            "SELECT namespace, key, value, created_at, updated_at \
             FROM {table} WHERE namespace = $1 AND key = $2",
            table = self.config.table_name,
        );

        let row: Option<(String, String, Value, String, String)> = sqlx::query_as(&sql)
            .bind(&ns)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgStore get error: {e}")))?;

        Ok(row.map(|(ns_str, k, value, created_at, updated_at)| Item {
            namespace: ns_str.split(':').map(String::from).collect(),
            key: k,
            value,
            created_at,
            updated_at,
            score: None,
        }))
    }

    async fn search(
        &self,
        namespace: &[&str],
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Item>, SynapticError> {
        validate_table_name(&self.config.table_name)?;
        let ns = encode_namespace(namespace);
        let limit = limit as i64;

        let rows: Vec<(String, String, Value, String, String)> = match query {
            Some(q) => {
                // Try full-text search via tsvector first.
                let fts_sql = format!(
                    "SELECT namespace, key, value, created_at, updated_at \
                     FROM {table} \
                     WHERE namespace = $1 AND tsv @@ plainto_tsquery('simple', $2) \
                     LIMIT $3",
                    table = self.config.table_name,
                );

                let fts_result: Result<Vec<(String, String, Value, String, String)>, _> =
                    sqlx::query_as(&fts_sql)
                        .bind(&ns)
                        .bind(q)
                        .bind(limit)
                        .fetch_all(&self.pool)
                        .await;

                match fts_result {
                    Ok(rows) => rows,
                    Err(_) => {
                        // Fall back to LIKE if tsvector is unavailable.
                        let like_pattern = format!("%{q}%");
                        let like_sql = format!(
                            "SELECT namespace, key, value, created_at, updated_at \
                             FROM {table} \
                             WHERE namespace = $1 AND (key LIKE $2 OR value::text LIKE $2) \
                             LIMIT $3",
                            table = self.config.table_name,
                        );

                        sqlx::query_as(&like_sql)
                            .bind(&ns)
                            .bind(&like_pattern)
                            .bind(limit)
                            .fetch_all(&self.pool)
                            .await
                            .map_err(|e| {
                                SynapticError::Store(format!("PgStore search error: {e}"))
                            })?
                    }
                }
            }
            None => {
                let sql = format!(
                    "SELECT namespace, key, value, created_at, updated_at \
                     FROM {table} WHERE namespace = $1 LIMIT $2",
                    table = self.config.table_name,
                );

                sqlx::query_as(&sql)
                    .bind(&ns)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| SynapticError::Store(format!("PgStore search error: {e}")))?
            }
        };

        let items = rows
            .into_iter()
            .map(|(ns_str, k, value, created_at, updated_at)| Item {
                namespace: ns_str.split(':').map(String::from).collect(),
                key: k,
                value,
                created_at,
                updated_at,
                score: None,
            })
            .collect();

        Ok(items)
    }

    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;
        let ns = encode_namespace(namespace);
        let now = now_iso();

        // Upsert: on insert both timestamps are set; on conflict only
        // updated_at is overwritten, preserving the original created_at.
        let sql = format!(
            "INSERT INTO {table} (namespace, key, value, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $4) \
             ON CONFLICT (namespace, key) DO UPDATE SET \
                 value = EXCLUDED.value, \
                 updated_at = EXCLUDED.updated_at",
            table = self.config.table_name,
        );

        sqlx::query(&sql)
            .bind(&ns)
            .bind(key)
            .bind(&value)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgStore put error: {e}")))?;

        Ok(())
    }

    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;
        let ns = encode_namespace(namespace);

        let sql = format!(
            "DELETE FROM {table} WHERE namespace = $1 AND key = $2",
            table = self.config.table_name,
        );

        sqlx::query(&sql)
            .bind(&ns)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Store(format!("PgStore delete error: {e}")))?;

        Ok(())
    }

    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let prefix_str = if prefix.is_empty() {
            String::new()
        } else {
            prefix.join(":")
        };

        let raw_namespaces: Vec<(String,)> = if prefix_str.is_empty() {
            let sql = format!(
                "SELECT DISTINCT namespace FROM {table}",
                table = self.config.table_name,
            );
            sqlx::query_as(&sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SynapticError::Store(format!("PgStore list_namespaces error: {e}")))?
        } else {
            let like_pattern = format!("{prefix_str}%");
            let sql = format!(
                "SELECT DISTINCT namespace FROM {table} WHERE namespace LIKE $1",
                table = self.config.table_name,
            );
            sqlx::query_as(&sql)
                .bind(&like_pattern)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SynapticError::Store(format!("PgStore list_namespaces error: {e}")))?
        };

        let namespaces: Vec<Vec<String>> = raw_namespaces
            .into_iter()
            .map(|(ns,)| ns.split(':').map(String::from).collect())
            .collect();

        Ok(namespaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_construction() {
        let config = PgStoreConfig::new("my_store");
        assert_eq!(config.table_name, "my_store");
    }

    #[test]
    fn validate_table_name_accepts_valid() {
        assert!(validate_table_name("synaptic_store").is_ok());
        assert!(validate_table_name("public.store").is_ok());
        assert!(validate_table_name("Store123").is_ok());
    }

    #[test]
    fn validate_table_name_rejects_invalid() {
        assert!(validate_table_name("").is_err());
        assert!(validate_table_name("store; DROP TABLE x").is_err());
        assert!(validate_table_name("store--evil").is_err());
        assert!(validate_table_name("store'bad").is_err());
    }

    #[test]
    fn encode_namespace_joins_with_colons() {
        assert_eq!(encode_namespace(&["a", "b", "c"]), "a:b:c");
        assert_eq!(encode_namespace(&[]), "");
        assert_eq!(encode_namespace(&["single"]), "single");
    }

    #[test]
    fn now_iso_is_non_empty() {
        let ts = now_iso();
        assert!(!ts.is_empty());
    }
}
