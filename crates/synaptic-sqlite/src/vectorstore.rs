use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::Connection;
use serde_json::Value;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

/// Configuration for [`SqliteVectorStore`].
#[derive(Debug, Clone)]
pub struct SqliteVectorStoreConfig {
    /// Path to the SQLite database file. Use `":memory:"` for an in-memory database.
    pub path: String,
}

impl SqliteVectorStoreConfig {
    /// Create a new configuration with a file path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Create a configuration for an in-memory SQLite database.
    pub fn in_memory() -> Self {
        Self {
            path: ":memory:".to_string(),
        }
    }
}

/// SQLite-backed implementation of the [`VectorStore`] trait.
///
/// Stores document embeddings as BLOBs (little-endian f32 sequences) and
/// computes cosine similarity in Rust. An FTS5 virtual table provides
/// full-text search for [`hybrid_search`](SqliteVectorStore::hybrid_search).
pub struct SqliteVectorStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteVectorStore {
    /// Create a new `SqliteVectorStore` from the given configuration.
    ///
    /// Opens (or creates) the SQLite database and initializes the vectors
    /// and FTS5 tables if they do not already exist.
    pub fn new(config: SqliteVectorStoreConfig) -> Result<Self, SynapticError> {
        let conn = Connection::open(&config.path)
            .map_err(|e| SynapticError::VectorStore(format!("SQLite open error: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS synaptic_vectors (
                id        TEXT PRIMARY KEY,
                content   TEXT NOT NULL,
                metadata  TEXT NOT NULL DEFAULT '{}',
                embedding BLOB
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS synaptic_vectors_fts USING fts5(
                content, id UNINDEXED
            );",
        )
        .map_err(|e| SynapticError::VectorStore(format!("SQLite create table error: {e}")))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Hybrid search combining cosine similarity and BM25 full-text scoring.
    ///
    /// `alpha` controls the balance:
    /// - `1.0` = pure vector similarity
    /// - `0.0` = pure BM25 text relevance
    /// - `0.5` = balanced (typical default)
    ///
    /// The final score is `alpha * cosine + (1 - alpha) * normalized_bm25`.
    pub async fn hybrid_search(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
        alpha: f32,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let query_vec = embeddings.embed_query(query).await?;
        let conn = self.conn.clone();
        let query = query.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::VectorStore(format!("lock error: {e}")))?;

            // Step 1: FTS5 MATCH to get text-relevant docs with BM25 scores.
            // bm25() returns negative values (closer to 0 = better match).
            let fts_results: HashMap<String, f64> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, bm25(synaptic_vectors_fts) as score
                         FROM synaptic_vectors_fts WHERE synaptic_vectors_fts MATCH ?1",
                    )
                    .map_err(|e| {
                        SynapticError::VectorStore(format!("SQLite FTS prepare error: {e}"))
                    })?;

                let rows: Vec<(String, f64)> = stmt
                    .query_map(rusqlite::params![query], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                    })
                    .map_err(|e| {
                        SynapticError::VectorStore(format!("SQLite FTS query error: {e}"))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                rows.into_iter().collect()
            };

            // Step 2: Load all docs with embeddings for cosine scoring.
            let mut stmt = conn
                .prepare(
                    "SELECT id, content, metadata, embedding FROM synaptic_vectors
                     WHERE embedding IS NOT NULL",
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite prepare error: {e}")))?;

            let all_docs: Vec<(Document, Vec<f32>)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })
                .map_err(|e| SynapticError::VectorStore(format!("SQLite query error: {e}")))?
                .filter_map(|r| r.ok())
                .map(|(id, content, meta_str, blob)| {
                    let metadata: HashMap<String, Value> =
                        serde_json::from_str(&meta_str).unwrap_or_default();
                    let embedding = blob_to_embed(&blob);
                    (
                        Document {
                            id,
                            content,
                            metadata,
                        },
                        embedding,
                    )
                })
                .collect();

            // Normalize BM25 scores to [0, 1] range.
            // bm25() returns negative values; we negate and normalize.
            let bm25_max = fts_results
                .values()
                .map(|s| -s) // negate: higher = better
                .fold(f64::NEG_INFINITY, f64::max);
            let bm25_max = if bm25_max <= 0.0 { 1.0 } else { bm25_max };

            // Step 3: Compute hybrid scores.
            let mut scored: Vec<(Document, f32)> = all_docs
                .into_iter()
                .map(|(doc, emb)| {
                    let cosine = cosine_similarity(&query_vec, &emb);
                    let bm25_raw = fts_results.get(&doc.id).copied().unwrap_or(0.0);
                    let bm25_normalized = (-bm25_raw / bm25_max) as f32;
                    let score = alpha * cosine + (1.0 - alpha) * bm25_normalized;
                    (doc, score)
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(k);

            Ok(scored)
        })
        .await
        .map_err(|e| SynapticError::VectorStore(format!("spawn_blocking error: {e}")))?
    }
}

#[async_trait]
impl VectorStore for SqliteVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError> {
        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let conn = self.conn.clone();

        let docs_with_vecs: Vec<(Document, Vec<f32>)> = docs.into_iter().zip(vectors).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::VectorStore(format!("lock error: {e}")))?;

            let mut ids = Vec::with_capacity(docs_with_vecs.len());

            for (mut doc, embedding) in docs_with_vecs {
                // Auto-assign UUID if id is empty.
                if doc.id.is_empty() {
                    doc.id = uuid::Uuid::new_v4().to_string();
                }

                let meta_str = serde_json::to_string(&doc.metadata).map_err(|e| {
                    SynapticError::VectorStore(format!("JSON serialize error: {e}"))
                })?;
                let blob = embed_to_blob(&embedding);

                conn.execute(
                    "INSERT OR REPLACE INTO synaptic_vectors (id, content, metadata, embedding)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![doc.id, doc.content, meta_str, blob],
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite insert error: {e}")))?;

                // Sync FTS: delete old then insert new.
                conn.execute(
                    "DELETE FROM synaptic_vectors_fts WHERE id = ?1",
                    rusqlite::params![doc.id],
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite FTS delete error: {e}")))?;

                conn.execute(
                    "INSERT INTO synaptic_vectors_fts (content, id) VALUES (?1, ?2)",
                    rusqlite::params![doc.content, doc.id],
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite FTS insert error: {e}")))?;

                ids.push(doc.id);
            }

            Ok(ids)
        })
        .await
        .map_err(|e| SynapticError::VectorStore(format!("spawn_blocking error: {e}")))?
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
        self.similarity_search_by_vector_with_score(&query_vec, k)
            .await
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
        let conn = self.conn.clone();
        let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::VectorStore(format!("lock error: {e}")))?;

            for id in &ids {
                conn.execute(
                    "DELETE FROM synaptic_vectors WHERE id = ?1",
                    rusqlite::params![id],
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite delete error: {e}")))?;

                conn.execute(
                    "DELETE FROM synaptic_vectors_fts WHERE id = ?1",
                    rusqlite::params![id],
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite FTS delete error: {e}")))?;
            }

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::VectorStore(format!("spawn_blocking error: {e}")))?
    }
}

impl SqliteVectorStore {
    /// Internal: similarity search by vector returning scores.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let conn = self.conn.clone();
        let query_vec = embedding.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::VectorStore(format!("lock error: {e}")))?;

            let mut stmt = conn
                .prepare(
                    "SELECT id, content, metadata, embedding FROM synaptic_vectors
                     WHERE embedding IS NOT NULL",
                )
                .map_err(|e| SynapticError::VectorStore(format!("SQLite prepare error: {e}")))?;

            let mut scored: Vec<(Document, f32)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })
                .map_err(|e| SynapticError::VectorStore(format!("SQLite query error: {e}")))?
                .filter_map(|r| r.ok())
                .map(|(id, content, meta_str, blob)| {
                    let metadata: HashMap<String, Value> =
                        serde_json::from_str(&meta_str).unwrap_or_default();
                    let embedding = blob_to_embed(&blob);
                    let score = cosine_similarity(&query_vec, &embedding);
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

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(k);

            Ok(scored)
        })
        .await
        .map_err(|e| SynapticError::VectorStore(format!("spawn_blocking error: {e}")))?
    }
}

/// Serialize an embedding vector to a little-endian byte blob.
fn embed_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize a little-endian byte blob to an embedding vector.
fn blob_to_embed(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}
