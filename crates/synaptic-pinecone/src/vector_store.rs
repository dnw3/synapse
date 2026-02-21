use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

// ---------------------------------------------------------------------------
// PineconeConfig
// ---------------------------------------------------------------------------

/// Configuration for connecting to a Pinecone index.
#[derive(Debug, Clone)]
pub struct PineconeConfig {
    /// Pinecone API key.
    pub api_key: String,
    /// The index host URL (e.g. `https://my-index-abc123.svc.pinecone.io`).
    pub host: String,
    /// Optional namespace for partitioning vectors within the index.
    pub namespace: Option<String>,
}

impl PineconeConfig {
    /// Create a new config with the required parameters.
    pub fn new(api_key: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            host: host.into(),
            namespace: None,
        }
    }

    /// Set the namespace for vector operations.
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }
}

// ---------------------------------------------------------------------------
// PineconeVectorStore
// ---------------------------------------------------------------------------

/// A [`VectorStore`] implementation backed by [Pinecone](https://www.pinecone.io/).
///
/// Uses the Pinecone REST API for all operations. Each document is stored as a
/// vector with:
/// - **id**: the document ID (auto-generated UUID v4 if empty)
/// - **values**: the embedding vector
/// - **metadata**: includes `content` (the document text) plus all document metadata
pub struct PineconeVectorStore {
    config: PineconeConfig,
    client: reqwest::Client,
}

impl PineconeVectorStore {
    /// Create a new store with the given configuration.
    pub fn new(config: PineconeConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &PineconeConfig {
        &self.config
    }

    /// Build the full URL for an API endpoint.
    fn url(&self, path: &str) -> String {
        let host = self.config.host.trim_end_matches('/');
        format!("{host}{path}")
    }

    /// Create a JSON body that includes the namespace field if configured.
    fn with_namespace(&self, mut body: serde_json::Value) -> serde_json::Value {
        if let Some(ref ns) = self.config.namespace {
            body["namespace"] = Value::String(ns.clone());
        }
        body
    }

    /// Send a POST request to the Pinecone API.
    async fn post(&self, path: &str, body: serde_json::Value) -> Result<Value, SynapticError> {
        let response = self
            .client
            .post(self.url(path))
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Pinecone request failed: {e}")))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("failed to read response: {e}")))?;

        if !status.is_success() {
            return Err(SynapticError::VectorStore(format!(
                "Pinecone API error (HTTP {status}): {text}"
            )));
        }

        serde_json::from_str(&text).map_err(|e| {
            SynapticError::VectorStore(format!("failed to parse Pinecone response: {e}"))
        })
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for PineconeVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError> {
        if docs.is_empty() {
            return Ok(Vec::new());
        }

        // Compute embeddings for all documents.
        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let mut ids = Vec::with_capacity(docs.len());
        let mut pinecone_vectors = Vec::with_capacity(docs.len());

        for (doc, vector) in docs.into_iter().zip(vectors) {
            let id = if doc.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                doc.id.clone()
            };

            // Build metadata: store the document content plus all existing metadata.
            let mut metadata = serde_json::Map::new();
            metadata.insert("content".to_string(), Value::String(doc.content));
            for (k, v) in doc.metadata {
                metadata.insert(k, v);
            }

            pinecone_vectors.push(serde_json::json!({
                "id": id,
                "values": vector,
                "metadata": metadata,
            }));

            ids.push(id);
        }

        let body = self.with_namespace(serde_json::json!({
            "vectors": pinecone_vectors,
        }));

        self.post("/vectors/upsert", body).await?;

        Ok(ids)
    }

    async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapticError> {
        let results = self.similarity_search_with_score(query, k, embeddings).await?;
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
        if ids.is_empty() {
            return Ok(());
        }

        let id_values: Vec<Value> = ids.iter().map(|id| Value::String(id.to_string())).collect();
        let body = self.with_namespace(serde_json::json!({
            "ids": id_values,
        }));

        self.post("/vectors/delete", body).await?;

        Ok(())
    }
}

impl PineconeVectorStore {
    /// Search by vector and return documents with their similarity scores.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let body = self.with_namespace(serde_json::json!({
            "vector": embedding,
            "topK": k,
            "includeMetadata": true,
        }));

        let response = self.post("/query", body).await?;

        let matches = response
            .get("matches")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();

        let mut results = Vec::with_capacity(matches.len());

        for m in matches {
            let id = m
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let score = m
                .get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            let metadata_obj = m
                .get("metadata")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            // Extract document content from metadata.
            let content = metadata_obj
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Build document metadata (exclude the "content" key).
            let metadata: HashMap<String, Value> = metadata_obj
                .into_iter()
                .filter(|(k, _)| k != "content")
                .collect();

            let doc = Document::with_metadata(id, content, metadata);
            results.push((doc, score));
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_fields() {
        let config = PineconeConfig::new("my-api-key", "https://my-index.svc.pinecone.io");
        assert_eq!(config.api_key, "my-api-key");
        assert_eq!(config.host, "https://my-index.svc.pinecone.io");
        assert!(config.namespace.is_none());
    }

    #[test]
    fn config_with_namespace() {
        let config = PineconeConfig::new("key", "https://host.pinecone.io")
            .with_namespace("my-namespace");
        assert_eq!(config.namespace.as_deref(), Some("my-namespace"));
    }

    #[test]
    fn config_builder_chain() {
        let config = PineconeConfig::new("key123", "https://idx.svc.pinecone.io")
            .with_namespace("production");

        assert_eq!(config.api_key, "key123");
        assert_eq!(config.host, "https://idx.svc.pinecone.io");
        assert_eq!(config.namespace.as_deref(), Some("production"));
    }

    #[test]
    fn store_new_creates_instance() {
        let config = PineconeConfig::new("key", "https://host.pinecone.io");
        let store = PineconeVectorStore::new(config);
        assert_eq!(store.config().api_key, "key");
        assert_eq!(store.config().host, "https://host.pinecone.io");
    }

    #[test]
    fn url_construction() {
        let config = PineconeConfig::new("key", "https://my-index.svc.pinecone.io");
        let store = PineconeVectorStore::new(config);
        assert_eq!(
            store.url("/vectors/upsert"),
            "https://my-index.svc.pinecone.io/vectors/upsert"
        );
    }

    #[test]
    fn url_construction_trailing_slash() {
        let config = PineconeConfig::new("key", "https://my-index.svc.pinecone.io/");
        let store = PineconeVectorStore::new(config);
        assert_eq!(
            store.url("/vectors/query"),
            "https://my-index.svc.pinecone.io/vectors/query"
        );
    }

    #[test]
    fn with_namespace_adds_field() {
        let config = PineconeConfig::new("key", "https://host.pinecone.io")
            .with_namespace("test-ns");
        let store = PineconeVectorStore::new(config);

        let body = store.with_namespace(serde_json::json!({"vector": [1.0]}));
        assert_eq!(body["namespace"], "test-ns");
    }

    #[test]
    fn with_namespace_omits_when_none() {
        let config = PineconeConfig::new("key", "https://host.pinecone.io");
        let store = PineconeVectorStore::new(config);

        let body = store.with_namespace(serde_json::json!({"vector": [1.0]}));
        assert!(body.get("namespace").is_none());
    }
}
