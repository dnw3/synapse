use std::collections::HashMap;

use async_trait::async_trait;
use pgvector::Vector;
use serde_json::Value;
use sqlx::PgPool;
use synaptic_core::{validate_table_name, Document, Embeddings, SynapticError, VectorStore};
use uuid::Uuid;

/// Configuration for a [`PgVectorStore`] table.
#[derive(Debug, Clone)]
pub struct PgVectorConfig {
    /// Name of the PostgreSQL table used to store documents and embeddings.
    pub table_name: String,
    /// Dimensionality of the embedding vectors (e.g. 1536 for OpenAI
    /// `text-embedding-ada-002`).
    pub vector_dimensions: u32,
}

impl PgVectorConfig {
    /// Create a new configuration.
    ///
    /// # Panics
    ///
    /// Panics if `table_name` is empty or `vector_dimensions` is zero.
    pub fn new(table_name: impl Into<String>, vector_dimensions: u32) -> Self {
        let table_name = table_name.into();
        assert!(!table_name.is_empty(), "table_name must not be empty");
        assert!(vector_dimensions > 0, "vector_dimensions must be > 0");
        Self {
            table_name,
            vector_dimensions,
        }
    }
}

/// A [`VectorStore`] backed by PostgreSQL with the pgvector extension.
///
/// Documents are stored in a single table with columns:
/// - `id TEXT PRIMARY KEY`
/// - `content TEXT NOT NULL`
/// - `metadata JSONB NOT NULL DEFAULT '{}'`
/// - `embedding vector(<dimensions>)`
///
/// Call [`initialize`](PgVectorStore::initialize) once after construction to
/// create the pgvector extension and the table (idempotent).
pub struct PgVectorStore {
    pool: PgPool,
    config: PgVectorConfig,
}

impl PgVectorStore {
    /// Create a new store from an existing connection pool and config.
    pub fn new(pool: PgPool, config: PgVectorConfig) -> Self {
        Self { pool, config }
    }

    /// Ensure the pgvector extension and the backing table exist.
    ///
    /// This is idempotent and safe to call on every application startup.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        // Validate the table name to prevent SQL injection. We only allow
        // alphanumeric characters, underscores, and dots (for schema-qualified
        // names).
        validate_table_name(&self.config.table_name)?;

        let create_ext = "CREATE EXTENSION IF NOT EXISTS vector";
        sqlx::query(create_ext)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                SynapticError::VectorStore(format!("failed to create pgvector extension: {e}"))
            })?;

        let create_table = format!(
            r#"CREATE TABLE IF NOT EXISTS {table} (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                metadata JSONB NOT NULL DEFAULT '{{}}',
                embedding vector({dims})
            )"#,
            table = self.config.table_name,
            dims = self.config.vector_dimensions,
        );
        sqlx::query(&create_table)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::VectorStore(format!("failed to create table: {e}")))?;

        Ok(())
    }

    /// Return a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &PgVectorConfig {
        &self.config
    }
}

#[async_trait]
impl VectorStore for PgVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError> {
        if docs.is_empty() {
            return Ok(Vec::new());
        }

        validate_table_name(&self.config.table_name)?;

        // Assign UUIDs where the caller has not provided an id.
        let docs: Vec<Document> = docs
            .into_iter()
            .map(|mut d| {
                if d.id.is_empty() {
                    d.id = Uuid::new_v4().to_string();
                }
                d
            })
            .collect();

        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let upsert_sql = format!(
            r#"INSERT INTO {table} (id, content, metadata, embedding)
               VALUES ($1, $2, $3, $4::vector)
               ON CONFLICT (id) DO UPDATE
               SET content = EXCLUDED.content,
                   metadata = EXCLUDED.metadata,
                   embedding = EXCLUDED.embedding"#,
            table = self.config.table_name,
        );

        let mut ids = Vec::with_capacity(docs.len());
        for (doc, vec) in docs.into_iter().zip(vectors) {
            let embedding = Vector::from(vec);
            let metadata = serde_json::to_value(&doc.metadata).map_err(|e| {
                SynapticError::VectorStore(format!("failed to serialize metadata: {e}"))
            })?;

            sqlx::query(&upsert_sql)
                .bind(&doc.id)
                .bind(&doc.content)
                .bind(&metadata)
                .bind(&embedding)
                .execute(&self.pool)
                .await
                .map_err(|e| SynapticError::VectorStore(format!("insert failed: {e}")))?;

            ids.push(doc.id);
        }

        Ok(ids)
    }

    async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapticError> {
        let results = self
            .similarity_search_with_score(query, k, embeddings)
            .await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let query_vec = embeddings.embed_query(query).await?;
        let raw = self
            .similarity_search_by_vector_with_score(&query_vec, k)
            .await?;
        Ok(raw)
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<Document>, SynapticError> {
        let results = self
            .similarity_search_by_vector_with_score(embedding, k)
            .await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapticError> {
        if ids.is_empty() {
            return Ok(());
        }

        validate_table_name(&self.config.table_name)?;

        let sql = format!(
            "DELETE FROM {table} WHERE id = ANY($1)",
            table = self.config.table_name,
        );

        let id_strings: Vec<String> = ids.iter().map(|s| s.to_string()).collect();

        sqlx::query(&sql)
            .bind(&id_strings)
            .execute(&self.pool)
            .await
            .map_err(|e| SynapticError::VectorStore(format!("delete failed: {e}")))?;

        Ok(())
    }
}

impl PgVectorStore {
    /// Internal helper that performs vector similarity search and returns
    /// documents together with their cosine similarity scores.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        validate_table_name(&self.config.table_name)?;

        let sql = format!(
            r#"SELECT id, content, metadata, 1 - (embedding <=> $1::vector) AS score
               FROM {table}
               ORDER BY embedding <=> $1::vector
               LIMIT $2"#,
            table = self.config.table_name,
        );

        let query_embedding = Vector::from(embedding.to_vec());

        let rows: Vec<(String, String, Value, f32)> = sqlx::query_as(&sql)
            .bind(&query_embedding)
            .bind(k as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SynapticError::VectorStore(format!("similarity search failed: {e}")))?;

        let results = rows
            .into_iter()
            .map(|(id, content, metadata, score)| {
                let metadata: HashMap<String, Value> = match metadata {
                    Value::Object(map) => map.into_iter().collect(),
                    _ => HashMap::new(),
                };
                (
                    Document {
                        id,
                        content,
                        metadata,
                    },
                    score,
                )
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_construction() {
        let config = PgVectorConfig::new("my_docs", 1536);
        assert_eq!(config.table_name, "my_docs");
        assert_eq!(config.vector_dimensions, 1536);
    }

    #[test]
    #[should_panic(expected = "table_name must not be empty")]
    fn config_rejects_empty_table_name() {
        PgVectorConfig::new("", 1536);
    }

    #[test]
    #[should_panic(expected = "vector_dimensions must be > 0")]
    fn config_rejects_zero_dimensions() {
        PgVectorConfig::new("docs", 0);
    }

    #[test]
    fn validate_table_name_accepts_valid_names() {
        assert!(validate_table_name("documents").is_ok());
        assert!(validate_table_name("my_docs").is_ok());
        assert!(validate_table_name("public.documents").is_ok());
        assert!(validate_table_name("schema1.table2").is_ok());
    }

    #[test]
    fn validate_table_name_rejects_sql_injection() {
        assert!(validate_table_name("docs; DROP TABLE users").is_err());
        assert!(validate_table_name("docs--comment").is_err());
        assert!(validate_table_name("docs'malicious").is_err());
        assert!(validate_table_name("").is_err());
    }
}
