//! Milvus vector store integration for Synaptic.
//!
//! This crate provides [`MilvusVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [Milvus](https://milvus.io/) using its REST API v2.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_milvus::{MilvusConfig, MilvusVectorStore};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = MilvusConfig::new("http://localhost:19530", "my_collection", 1536);
//! let store = MilvusVectorStore::new(config);
//! store.initialize().await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

/// Configuration for connecting to a Milvus instance.
#[derive(Debug, Clone)]
pub struct MilvusConfig {
    /// Milvus endpoint URL (e.g., `http://localhost:19530`).
    pub endpoint: String,
    /// Collection name to store documents in.
    pub collection: String,
    /// Optional API key for Zilliz Cloud authentication.
    pub api_key: Option<String>,
    /// Vector dimension (must match your embedding model).
    pub dim: usize,
}

impl MilvusConfig {
    /// Create a new configuration with required fields.
    pub fn new(endpoint: impl Into<String>, collection: impl Into<String>, dim: usize) -> Self {
        Self {
            endpoint: endpoint.into(),
            collection: collection.into(),
            api_key: None,
            dim,
        }
    }

    /// Set the API key for Zilliz Cloud or secured Milvus instances.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
}

/// A [`VectorStore`] implementation backed by [Milvus](https://milvus.io/).
///
/// Uses the Milvus REST API v2. Call
/// [`initialize`](MilvusVectorStore::initialize) to create the collection
/// before inserting documents.
pub struct MilvusVectorStore {
    config: MilvusConfig,
    client: reqwest::Client,
}

impl MilvusVectorStore {
    /// Create a new store with the given configuration.
    pub fn new(config: MilvusConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &MilvusConfig {
        &self.config
    }

    /// Create the Milvus collection if it does not already exist.
    ///
    /// This is idempotent — calling it when the collection already exists
    /// is a no-op.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        let body = json!({
            "collectionName": self.config.collection,
            "dimension": self.config.dim,
            "metricType": "COSINE",
        });
        let resp = self
            .request("POST", "/v2/vectordb/collections/create", &body)
            .await?;

        let code = resp["code"].as_i64().unwrap_or(0);
        if code != 0 {
            let msg = resp["message"].as_str().unwrap_or("");
            // Treat "already exist" as success.
            if !msg.to_lowercase().contains("already exist") {
                return Err(SynapticError::VectorStore(format!(
                    "Milvus create collection error (code {code}): {msg}"
                )));
            }
        }
        Ok(())
    }

    /// Send a JSON request to the Milvus REST API.
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Value, SynapticError> {
        let url = format!("{}{}", self.config.endpoint.trim_end_matches('/'), path);
        let mut req = match method {
            "POST" => self.client.post(&url),
            "DELETE" => self.client.delete(&url),
            _ => self.client.get(&url),
        };
        req = req.header("Content-Type", "application/json");
        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        let resp = req
            .json(body)
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Milvus request error: {e}")))?;
        let status = resp.status().as_u16();
        let json: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Milvus response parse error: {e}")))?;
        if status >= 400 {
            return Err(SynapticError::VectorStore(format!(
                "Milvus HTTP error ({status}): {json}"
            )));
        }
        Ok(json)
    }

    /// Search by raw vector and return documents with their similarity scores.
    async fn search_by_vector_with_score(
        &self,
        vector: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let body = json!({
            "collectionName": self.config.collection,
            "data": [vector],
            "limit": k,
            "outputFields": ["docId", "content", "metadata"],
        });
        let resp = self
            .request("POST", "/v2/vectordb/entities/search", &body)
            .await?;

        let results = resp["data"].as_array().cloned().unwrap_or_default();
        let docs = results
            .iter()
            .filter_map(|r| {
                let doc_id = r["docId"].as_str()?.to_string();
                let content = r["content"].as_str()?.to_string();
                let metadata_str = r["metadata"].as_str().unwrap_or("{}");
                let metadata: HashMap<String, Value> =
                    serde_json::from_str(metadata_str).unwrap_or_default();
                let score = r["distance"].as_f64().unwrap_or(0.0) as f32;
                Some((Document::with_metadata(doc_id, content, metadata), score))
            })
            .collect();
        Ok(docs)
    }
}

#[async_trait]
impl VectorStore for MilvusVectorStore {
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

        let data: Vec<Value> = docs
            .iter()
            .zip(vectors.iter())
            .map(|(doc, vec)| {
                let metadata_str =
                    serde_json::to_string(&doc.metadata).unwrap_or_else(|_| "{}".to_string());
                json!({
                    "docId": doc.id,
                    "content": doc.content,
                    "metadata": metadata_str,
                    "vector": vec,
                })
            })
            .collect();

        let body = json!({
            "collectionName": self.config.collection,
            "data": data,
        });
        self.request("POST", "/v2/vectordb/entities/insert", &body)
            .await?;

        Ok(docs.into_iter().map(|d| d.id).collect())
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
        self.search_by_vector_with_score(&qvec, k).await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<Document>, SynapticError> {
        let results = self.search_by_vector_with_score(embedding, k).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapticError> {
        if ids.is_empty() {
            return Ok(());
        }
        let filter = format!(
            "docId in [{}]",
            ids.iter()
                .map(|id| format!("\"{}\"", id))
                .collect::<Vec<_>>()
                .join(",")
        );
        let body = json!({
            "collectionName": self.config.collection,
            "filter": filter,
        });
        self.request("POST", "/v2/vectordb/entities/delete", &body)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_fields() {
        let config = MilvusConfig::new("http://localhost:19530", "test_collection", 1536);
        assert_eq!(config.endpoint, "http://localhost:19530");
        assert_eq!(config.collection, "test_collection");
        assert_eq!(config.dim, 1536);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn config_with_api_key() {
        let config =
            MilvusConfig::new("http://localhost:19530", "test", 768).with_api_key("my-token");
        assert_eq!(config.api_key, Some("my-token".to_string()));
    }

    #[test]
    fn store_new_creates_instance() {
        let config = MilvusConfig::new("http://localhost:19530", "coll", 512);
        let store = MilvusVectorStore::new(config);
        assert_eq!(store.config().collection, "coll");
        assert_eq!(store.config().dim, 512);
    }
}
