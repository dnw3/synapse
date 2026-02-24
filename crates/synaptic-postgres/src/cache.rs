use async_trait::async_trait;
use sqlx::PgPool;
use synaptic_core::{validate_table_name, ChatResponse, SynapticError};

/// Configuration for [`PgCache`].
#[derive(Debug, Clone)]
pub struct PgCacheConfig {
    /// Name of the PostgreSQL table used to store cached LLM responses.
    pub table_name: String,
    /// Optional TTL in seconds. When set, cached entries older than this are
    /// treated as expired and excluded from lookups.
    pub ttl: Option<u64>,
}

impl PgCacheConfig {
    /// Create a new configuration with the given table name.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            ttl: None,
        }
    }

    /// Set the TTL (time-to-live) in seconds for cached entries.
    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl = Some(seconds);
        self
    }
}

/// PostgreSQL-backed implementation of the [`LlmCache`](synaptic_core::LlmCache) trait.
///
/// Stores serialized [`ChatResponse`] values in a PostgreSQL table with optional
/// TTL expiration. Call [`initialize`](PgCache::initialize) once after construction
/// to create the backing table (idempotent).
///
/// # Example
///
/// ```rust,no_run
/// use sqlx::postgres::PgPoolOptions;
/// use synaptic_postgres::{PgCache, PgCacheConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPoolOptions::new()
///     .max_connections(5)
///     .connect("postgres://user:pass@localhost/mydb")
///     .await?;
///
/// let config = PgCacheConfig::new("llm_cache").with_ttl(3600);
/// let cache = PgCache::new(pool, config);
/// cache.initialize().await?;
/// # Ok(())
/// # }
/// ```
pub struct PgCache {
    pool: PgPool,
    config: PgCacheConfig,
}

impl PgCache {
    /// Create a new `PgCache` from an existing connection pool and config.
    pub fn new(pool: PgPool, config: PgCacheConfig) -> Self {
        Self { pool, config }
    }

    /// Ensure the backing table exists.
    ///
    /// This is idempotent and safe to call on every application startup.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let create_table = format!(
            r#"CREATE TABLE IF NOT EXISTS {table} (
                key        TEXT PRIMARY KEY,
                value      TEXT NOT NULL,
                created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
            )"#,
            table = self.config.table_name,
        );

        sqlx::query(&create_table)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Cache(format!("failed to create table: {e}")))?;

        Ok(())
    }

    /// Return a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &PgCacheConfig {
        &self.config
    }
}

#[async_trait]
impl synaptic_core::LlmCache for PgCache {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let json_str: Option<String> = if let Some(ttl) = self.config.ttl {
            let sql = format!(
                "SELECT value FROM {table} WHERE key = $1 AND created_at + $2 > EXTRACT(EPOCH FROM now())::BIGINT",
                table = self.config.table_name,
            );
            sqlx::query_scalar(&sql)
                .bind(key)
                .bind(ttl as i64)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SynapticError::Cache(format!("query error: {e}")))?
        } else {
            let sql = format!(
                "SELECT value FROM {table} WHERE key = $1",
                table = self.config.table_name,
            );
            sqlx::query_scalar(&sql)
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| SynapticError::Cache(format!("query error: {e}")))?
        };

        match json_str {
            Some(s) => {
                let response: ChatResponse = serde_json::from_str(&s)
                    .map_err(|e| SynapticError::Cache(format!("JSON deserialize error: {e}")))?;
                Ok(Some(response))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let value = serde_json::to_string(response)
            .map_err(|e| SynapticError::Cache(format!("JSON serialize error: {e}")))?;

        let sql = format!(
            r#"INSERT INTO {table} (key, value, created_at)
               VALUES ($1, $2, EXTRACT(EPOCH FROM now())::BIGINT)
               ON CONFLICT (key) DO UPDATE
               SET value = EXCLUDED.value,
                   created_at = EXCLUDED.created_at"#,
            table = self.config.table_name,
        );

        sqlx::query(&sql)
            .bind(key)
            .bind(&value)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Cache(format!("insert error: {e}")))?;

        Ok(())
    }

    async fn clear(&self) -> Result<(), SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let sql = format!("DELETE FROM {table}", table = self.config.table_name);

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::Cache(format!("delete error: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_construction() {
        let config = PgCacheConfig::new("my_cache");
        assert_eq!(config.table_name, "my_cache");
        assert!(config.ttl.is_none());
    }

    #[test]
    fn config_with_ttl() {
        let config = PgCacheConfig::new("my_cache").with_ttl(3600);
        assert_eq!(config.table_name, "my_cache");
        assert_eq!(config.ttl, Some(3600));
    }

    #[test]
    fn validate_table_name_accepts_valid_names() {
        assert!(validate_table_name("llm_cache").is_ok());
        assert!(validate_table_name("my_cache").is_ok());
        assert!(validate_table_name("public.llm_cache").is_ok());
        assert!(validate_table_name("schema1.cache2").is_ok());
    }

    #[test]
    fn validate_table_name_rejects_sql_injection() {
        assert!(validate_table_name("cache; DROP TABLE users").is_err());
        assert!(validate_table_name("cache--comment").is_err());
        assert!(validate_table_name("cache'malicious").is_err());
        assert!(validate_table_name("").is_err());
    }
}
