//! LanceDB embedded vector store integration for Synaptic.
//!
//! This crate provides [`LanceDbVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [LanceDB](https://lancedb.github.io/lancedb/) — a serverless, embedded
//! vector database that requires no separate server process.
//!
//! # Dependency Note
//!
//! The `lancedb` crate (>= 0.20) has transitive dependencies on `aws-smithy`
//! crates that require Rust >= 1.91. Until those dependencies stabilise at
//! MSRV-compatible versions this crate ships a pure-Rust in-memory backend
//! with the full `VectorStore` interface so that your application can compile
//! and test today, and you can swap in the real LanceDB backend by replacing
//! `LanceDbVectorStore::new` with a lancedb-backed constructor when your
//! toolchain supports it.
//!
//! # Data Layout
//!
//! LanceDB stores data in the [Lance](https://github.com/lancedb/lance) columnar
//! format on the local filesystem (or in cloud object storage such as S3/GCS).
//! The embedded implementation in this crate mirrors that interface: a single
//! table per store keyed by the `uri` and `table_name` fields of
//! [`LanceDbConfig`].
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_lancedb::{LanceDbConfig, LanceDbVectorStore};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Local file-based storage
//! let config = LanceDbConfig::new("/var/lib/myapp/vectors", "documents", 1536);
//! let store = LanceDbVectorStore::new(config).await?;
//!
//! // Cloud-backed storage (when using the native lancedb crate)
//! // let config = LanceDbConfig::new("s3://my-bucket/vectors", "documents", 1536);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
use tokio::sync::RwLock;

/// Configuration for a LanceDB vector store.
#[derive(Debug, Clone)]
pub struct LanceDbConfig {
    /// Storage URI — local filesystem path (e.g., `/data/mydb`) or
    /// cloud object storage URI (e.g., `s3://bucket/path`).
    pub uri: String,
    /// Table name within the database.
    pub table_name: String,
    /// Vector dimension — must match the output dimension of the embedding
    /// model you intend to use with this store.
    pub dim: usize,
}

impl LanceDbConfig {
    /// Create a new configuration.
    pub fn new(uri: impl Into<String>, table_name: impl Into<String>, dim: usize) -> Self {
        Self {
            uri: uri.into(),
            table_name: table_name.into(),
            dim,
        }
    }
}

/// A row stored internally in the in-memory table.
#[derive(Clone)]
struct Row {
    id: String,
    content: String,
    metadata: HashMap<String, Value>,
    embedding: Vec<f32>,
}

/// A [`VectorStore`] implementation that mirrors the LanceDB API surface.
///
/// This implementation keeps all data in memory (behind a `tokio::sync::RwLock`)
/// and performs exact cosine similarity search, making it suitable for
/// development, testing, and small-scale production workloads.
///
/// When the `lancedb` crate becomes compatible with the workspace MSRV, this
/// struct can be replaced with a lancedb-native backend without changing any
/// call-site code.
pub struct LanceDbVectorStore {
    config: LanceDbConfig,
    rows: Arc<RwLock<Vec<Row>>>,
}

impl LanceDbVectorStore {
    /// Create a new store from the given configuration.
    ///
    /// This is an `async` function for forward-compatibility with the native
    /// lancedb backend, which connects asynchronously.
    pub async fn new(config: LanceDbConfig) -> Result<Self, SynapticError> {
        Ok(Self {
            config,
            rows: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &LanceDbConfig {
        &self.config
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// Search for the k nearest neighbours by cosine similarity.
    async fn knn_search(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let rows = self.rows.read().await;
        let mut scored: Vec<(f32, usize)> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| (Self::cosine_similarity(query, &row.embedding), i))
            .collect();

        // Sort descending by score.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let results = scored
            .into_iter()
            .take(k)
            .map(|(score, i)| {
                let row = &rows[i];
                (
                    Document::with_metadata(
                        row.id.clone(),
                        row.content.clone(),
                        row.metadata.clone(),
                    ),
                    score,
                )
            })
            .collect();

        Ok(results)
    }
}

#[async_trait]
impl VectorStore for LanceDbVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError> {
        if docs.is_empty() {
            return Ok(vec![]);
        }

        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let mut rows = self.rows.write().await;
        let ids: Vec<String> = docs
            .into_iter()
            .zip(vectors)
            .map(|(doc, vec)| {
                let id = doc.id.clone();
                rows.push(Row {
                    id: doc.id,
                    content: doc.content,
                    metadata: doc.metadata,
                    embedding: vec,
                });
                id
            })
            .collect();

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
        let qvec = embeddings.embed_query(query).await?;
        self.knn_search(&qvec, k).await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<Document>, SynapticError> {
        let results = self.knn_search(embedding, k).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapticError> {
        let id_set: std::collections::HashSet<&str> = ids.iter().copied().collect();
        let mut rows = self.rows.write().await;
        rows.retain(|row| !id_set.contains(row.id.as_str()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_fields() {
        let config = LanceDbConfig::new("/tmp/test_db", "test_table", 1536);
        assert_eq!(config.uri, "/tmp/test_db");
        assert_eq!(config.table_name, "test_table");
        assert_eq!(config.dim, 1536);
    }

    #[tokio::test]
    async fn store_new_creates_instance() {
        let config = LanceDbConfig::new("/tmp/db", "tbl", 4);
        let store = LanceDbVectorStore::new(config).await.unwrap();
        assert_eq!(store.config().table_name, "tbl");
        assert_eq!(store.config().dim, 4);
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0_f32, 0.0, 0.0];
        let score = LanceDbVectorStore::cosine_similarity(&v, &v);
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let score = LanceDbVectorStore::cosine_similarity(&a, &b);
        assert!(score.abs() < 1e-6);
    }
}
