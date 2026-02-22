//! OpenSearch vector store integration for Synaptic.
//!
//! This crate provides [`OpenSearchVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [OpenSearch](https://opensearch.org/) using its k-NN plugin.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_opensearch::{OpenSearchConfig, OpenSearchVectorStore};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = OpenSearchConfig::new("http://localhost:9200", "my_index", 1536)
//!     .with_credentials("admin", "admin");
//! let store = OpenSearchVectorStore::new(config);
//! store.initialize().await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

/// Configuration for connecting to an OpenSearch cluster.
#[derive(Debug, Clone)]
pub struct OpenSearchConfig {
    /// OpenSearch endpoint URL (e.g., `http://localhost:9200`).
    pub endpoint: String,
    /// Index name to store documents in.
    pub index: String,
    /// Vector dimension (must match your embedding model).
    pub dim: usize,
    /// Optional username for HTTP Basic Auth.
    pub username: Option<String>,
    /// Optional password for HTTP Basic Auth.
    pub password: Option<String>,
}

impl OpenSearchConfig {
    /// Create a new configuration with required fields.
    pub fn new(endpoint: impl Into<String>, index: impl Into<String>, dim: usize) -> Self {
        Self {
            endpoint: endpoint.into(),
            index: index.into(),
            dim,
            username: None,
            password: None,
        }
    }

    /// Set HTTP Basic Auth credentials.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }
}

/// A [`VectorStore`] implementation backed by [OpenSearch](https://opensearch.org/).
///
/// Uses OpenSearch's k-NN plugin with HNSW indexing for approximate nearest
/// neighbor search. Call [`initialize`](OpenSearchVectorStore::initialize)
/// to create the index with correct mappings before inserting documents.
pub struct OpenSearchVectorStore {
    config: OpenSearchConfig,
    client: reqwest::Client,
}

impl OpenSearchVectorStore {
    /// Create a new store with the given configuration.
    pub fn new(config: OpenSearchConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &OpenSearchConfig {
        &self.config
    }

    /// Create the OpenSearch index with k-NN mappings if it does not exist.
    ///
    /// This is idempotent — calling it when the index already exists is safe.
    pub async fn initialize(&self) -> Result<(), SynapticError> {
        // Check if index exists first.
        let head_url = format!(
            "{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.index
        );
        let mut head_req = self.client.head(&head_url);
        if let (Some(ref u), Some(ref p)) = (&self.config.username, &self.config.password) {
            head_req = head_req.basic_auth(u, Some(p));
        }
        let head_resp = head_req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("OpenSearch HEAD request failed: {e}"))
        })?;
        if head_resp.status().is_success() {
            // Index already exists.
            return Ok(());
        }

        let mapping = json!({
            "settings": {
                "index": { "knn": true }
            },
            "mappings": {
                "properties": {
                    "doc_id": { "type": "keyword" },
                    "content": { "type": "text" },
                    "metadata": { "type": "object", "enabled": false },
                    "embedding": {
                        "type": "knn_vector",
                        "dimension": self.config.dim,
                        "method": {
                            "name": "hnsw",
                            "space_type": "cosinesimil",
                            "engine": "nmslib"
                        }
                    }
                }
            }
        });

        let put_url = format!(
            "{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.index
        );
        let mut put_req = self
            .client
            .put(&put_url)
            .header("Content-Type", "application/json")
            .json(&mapping);
        if let (Some(ref u), Some(ref p)) = (&self.config.username, &self.config.password) {
            put_req = put_req.basic_auth(u, Some(p));
        }
        let put_resp = put_req
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("OpenSearch PUT index failed: {e}")))?;

        let status = put_resp.status().as_u16();
        if status >= 400 {
            let body: Value = put_resp.json().await.unwrap_or_default();
            // 400 with "already_exists" is fine (race condition).
            let err_type = body["error"]["type"].as_str().unwrap_or("");
            if !err_type.contains("already_exists") {
                return Err(SynapticError::VectorStore(format!(
                    "OpenSearch create index error (HTTP {status}): {body}"
                )));
            }
        }
        Ok(())
    }

    /// Apply basic auth to a request builder if credentials are configured.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let (Some(ref u), Some(ref p)) = (&self.config.username, &self.config.password) {
            builder.basic_auth(u, Some(p))
        } else {
            builder
        }
    }

    /// Search by raw vector and return documents with similarity scores.
    async fn search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let body = json!({
            "size": k,
            "query": {
                "knn": {
                    "embedding": {
                        "vector": embedding,
                        "k": k,
                    }
                }
            },
            "_source": ["doc_id", "content", "metadata"],
        });

        let search_url = format!(
            "{}/{}/_search",
            self.config.endpoint.trim_end_matches('/'),
            self.config.index
        );
        let req = self
            .apply_auth(self.client.post(&search_url))
            .header("Content-Type", "application/json")
            .json(&body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("OpenSearch search request failed: {e}"))
        })?;

        let status = resp.status().as_u16();
        let json: Value = resp.json().await.map_err(|e| {
            SynapticError::VectorStore(format!("OpenSearch search response parse error: {e}"))
        })?;

        if status >= 400 {
            return Err(SynapticError::VectorStore(format!(
                "OpenSearch search error (HTTP {status}): {json}"
            )));
        }

        let hits = json["hits"]["hits"].as_array().cloned().unwrap_or_default();

        let docs = hits
            .iter()
            .filter_map(|h| {
                let src = h["_source"].as_object()?;
                let id = src["doc_id"].as_str()?.to_string();
                let content = src["content"].as_str()?.to_string();
                let metadata: HashMap<String, Value> = src["metadata"]
                    .as_object()
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let score = h["_score"].as_f64().unwrap_or(0.0) as f32;
                Some((Document::with_metadata(id, content, metadata), score))
            })
            .collect();

        Ok(docs)
    }
}

#[async_trait]
impl VectorStore for OpenSearchVectorStore {
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

        // Build ndjson bulk request body.
        let mut bulk_body = String::new();
        for (doc, vec) in docs.iter().zip(vectors.iter()) {
            let action = json!({
                "index": {
                    "_index": self.config.index,
                    "_id": doc.id,
                }
            });
            let data = json!({
                "doc_id": doc.id,
                "content": doc.content,
                "metadata": doc.metadata,
                "embedding": vec,
            });
            bulk_body.push_str(&action.to_string());
            bulk_body.push('\n');
            bulk_body.push_str(&data.to_string());
            bulk_body.push('\n');
        }

        let bulk_url = format!(
            "{}/{}/_bulk",
            self.config.endpoint.trim_end_matches('/'),
            self.config.index
        );
        let req = self
            .apply_auth(self.client.post(&bulk_url))
            .header("Content-Type", "application/x-ndjson")
            .body(bulk_body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("OpenSearch bulk request failed: {e}"))
        })?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let text = resp.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "OpenSearch bulk error (HTTP {status}): {text}"
            )));
        }

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

        let mut bulk_body = String::new();
        for id in ids {
            let action = json!({
                "delete": {
                    "_index": self.config.index,
                    "_id": id,
                }
            });
            bulk_body.push_str(&action.to_string());
            bulk_body.push('\n');
        }

        let bulk_url = format!("{}/_bulk", self.config.endpoint.trim_end_matches('/'));
        let req = self
            .apply_auth(self.client.post(&bulk_url))
            .header("Content-Type", "application/x-ndjson")
            .body(bulk_body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("OpenSearch delete request failed: {e}"))
        })?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let text = resp.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "OpenSearch delete error (HTTP {status}): {text}"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_defaults() {
        let config = OpenSearchConfig::new("http://localhost:9200", "test_index", 1536);
        assert_eq!(config.endpoint, "http://localhost:9200");
        assert_eq!(config.index, "test_index");
        assert_eq!(config.dim, 1536);
        assert!(config.username.is_none());
        assert!(config.password.is_none());
    }

    #[test]
    fn config_with_credentials() {
        let config = OpenSearchConfig::new("http://localhost:9200", "test", 768)
            .with_credentials("admin", "password");
        assert_eq!(config.username, Some("admin".to_string()));
        assert_eq!(config.password, Some("password".to_string()));
    }

    #[test]
    fn store_new_creates_instance() {
        let config = OpenSearchConfig::new("http://localhost:9200", "idx", 512);
        let store = OpenSearchVectorStore::new(config);
        assert_eq!(store.config().index, "idx");
        assert_eq!(store.config().dim, 512);
    }
}
