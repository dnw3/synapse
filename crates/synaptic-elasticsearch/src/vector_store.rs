use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

// ---------------------------------------------------------------------------
// ElasticsearchConfig
// ---------------------------------------------------------------------------

/// Configuration for connecting to an Elasticsearch cluster.
#[derive(Debug, Clone)]
pub struct ElasticsearchConfig {
    /// Elasticsearch URL (default: `http://localhost:9200`).
    pub url: String,
    /// Name of the index to store documents in.
    pub index_name: String,
    /// Field name for storing embedding vectors (default: `embedding`).
    pub vector_field: String,
    /// Field name for storing document content (default: `content`).
    pub content_field: String,
    /// Vector dimensionality (required for index creation).
    pub dims: usize,
    /// Optional username for basic authentication.
    pub username: Option<String>,
    /// Optional password for basic authentication.
    pub password: Option<String>,
}

impl ElasticsearchConfig {
    /// Create a new config with the required index name and vector dimensions.
    ///
    /// Uses default values for URL (`http://localhost:9200`), vector field
    /// (`embedding`), and content field (`content`).
    pub fn new(index_name: impl Into<String>, dims: usize) -> Self {
        Self {
            url: "http://localhost:9200".to_string(),
            index_name: index_name.into(),
            vector_field: "embedding".to_string(),
            content_field: "content".to_string(),
            dims,
            username: None,
            password: None,
        }
    }

    /// Set the Elasticsearch URL.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Set the vector field name.
    pub fn with_vector_field(mut self, field: impl Into<String>) -> Self {
        self.vector_field = field.into();
        self
    }

    /// Set the content field name.
    pub fn with_content_field(mut self, field: impl Into<String>) -> Self {
        self.content_field = field.into();
        self
    }

    /// Set basic authentication credentials.
    pub fn with_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }
}

// ---------------------------------------------------------------------------
// ElasticsearchVectorStore
// ---------------------------------------------------------------------------

/// A [`VectorStore`] implementation backed by [Elasticsearch](https://www.elastic.co/).
///
/// Uses the Elasticsearch REST API with `dense_vector` field type and kNN
/// search for similarity queries. Documents are stored with:
/// - `_id`: the document ID
/// - `content`: the document text
/// - `embedding`: the vector (dense_vector type)
/// - `metadata`: an object field with arbitrary metadata
///
/// Call [`ensure_index`](ElasticsearchVectorStore::ensure_index) to create
/// the index with proper mappings before inserting documents.
pub struct ElasticsearchVectorStore {
    config: ElasticsearchConfig,
    client: reqwest::Client,
}

impl ElasticsearchVectorStore {
    /// Create a new store with the given configuration.
    pub fn new(config: ElasticsearchConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &ElasticsearchConfig {
        &self.config
    }

    /// Build a full URL for the given path.
    fn url(&self, path: &str) -> String {
        let base = self.config.url.trim_end_matches('/');
        format!("{base}{path}")
    }

    /// Apply basic auth to a request builder if credentials are configured.
    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let (Some(ref user), Some(ref pass)) = (&self.config.username, &self.config.password) {
            builder.basic_auth(user, Some(pass))
        } else {
            builder
        }
    }

    /// Ensure the index exists with the correct mappings.
    ///
    /// Creates the index if it does not exist. If the index already exists,
    /// this is a no-op. Idempotent and safe to call on every startup.
    pub async fn ensure_index(&self) -> Result<(), SynapticError> {
        let index_url = self.url(&format!("/{}", self.config.index_name));

        // Check if index exists.
        let head_req = self.apply_auth(self.client.head(&index_url));
        let head_resp = head_req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("Elasticsearch HEAD request failed: {e}"))
        })?;

        if head_resp.status().is_success() {
            // Index already exists.
            return Ok(());
        }

        // Create the index with mappings.
        let mappings = serde_json::json!({
            "mappings": {
                "properties": {
                    &self.config.content_field: {
                        "type": "text"
                    },
                    &self.config.vector_field: {
                        "type": "dense_vector",
                        "dims": self.config.dims,
                        "index": true,
                        "similarity": "cosine"
                    },
                    "metadata": {
                        "type": "object",
                        "enabled": false
                    }
                }
            }
        });

        let put_req = self
            .apply_auth(self.client.put(&index_url))
            .header("Content-Type", "application/json")
            .json(&mappings);

        let put_resp = put_req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("Elasticsearch PUT index failed: {e}"))
        })?;

        let status = put_resp.status();
        if !status.is_success() {
            let text = put_resp.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "Elasticsearch create index error (HTTP {status}): {text}"
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for ElasticsearchVectorStore {
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
        let mut bulk_body = String::new();

        for (doc, vector) in docs.into_iter().zip(vectors) {
            let id = if doc.id.is_empty() {
                generate_id()
            } else {
                doc.id.clone()
            };

            // Build the action line.
            let action = serde_json::json!({
                "index": {
                    "_index": self.config.index_name,
                    "_id": id,
                }
            });
            bulk_body.push_str(&action.to_string());
            bulk_body.push('\n');

            // Build the document line.
            let doc_body = serde_json::json!({
                &self.config.content_field: doc.content,
                &self.config.vector_field: vector,
                "metadata": doc.metadata,
            });
            bulk_body.push_str(&doc_body.to_string());
            bulk_body.push('\n');

            ids.push(id);
        }

        let bulk_url = self.url("/_bulk");
        let req = self
            .apply_auth(self.client.post(&bulk_url))
            .header("Content-Type", "application/x-ndjson")
            .body(bulk_body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("Elasticsearch bulk request failed: {e}"))
        })?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| {
            SynapticError::VectorStore(format!("failed to read Elasticsearch response: {e}"))
        })?;

        if !status.is_success() {
            return Err(SynapticError::VectorStore(format!(
                "Elasticsearch bulk error (HTTP {status}): {text}"
            )));
        }

        // Check for item-level errors in the bulk response.
        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            SynapticError::VectorStore(format!(
                "failed to parse Elasticsearch bulk response: {e}"
            ))
        })?;

        if parsed.get("errors").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(SynapticError::VectorStore(format!(
                "Elasticsearch bulk operation had errors: {text}"
            )));
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

        let mut bulk_body = String::new();
        for id in ids {
            let action = serde_json::json!({
                "delete": {
                    "_index": self.config.index_name,
                    "_id": id,
                }
            });
            bulk_body.push_str(&action.to_string());
            bulk_body.push('\n');
        }

        let bulk_url = self.url("/_bulk");
        let req = self
            .apply_auth(self.client.post(&bulk_url))
            .header("Content-Type", "application/x-ndjson")
            .body(bulk_body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("Elasticsearch delete request failed: {e}"))
        })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "Elasticsearch delete error (HTTP {status}): {text}"
            )));
        }

        Ok(())
    }
}

impl ElasticsearchVectorStore {
    /// Search by vector and return documents with their similarity scores.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let num_candidates = std::cmp::max(k * 10, 100);

        let search_body = serde_json::json!({
            "size": k,
            "knn": {
                "field": self.config.vector_field,
                "query_vector": embedding,
                "k": k,
                "num_candidates": num_candidates,
            },
            "_source": [&self.config.content_field, "metadata"],
        });

        let search_url = self.url(&format!("/{}/_search", self.config.index_name));
        let req = self
            .apply_auth(self.client.post(&search_url))
            .header("Content-Type", "application/json")
            .json(&search_body);

        let resp = req.send().await.map_err(|e| {
            SynapticError::VectorStore(format!("Elasticsearch search failed: {e}"))
        })?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| {
            SynapticError::VectorStore(format!("failed to read Elasticsearch response: {e}"))
        })?;

        if !status.is_success() {
            return Err(SynapticError::VectorStore(format!(
                "Elasticsearch search error (HTTP {status}): {text}"
            )));
        }

        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            SynapticError::VectorStore(format!(
                "failed to parse Elasticsearch response: {e}"
            ))
        })?;

        let hits = parsed["hits"]["hits"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let mut results = Vec::with_capacity(hits.len());

        for hit in hits {
            let id = hit
                .get("_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let score = hit
                .get("_score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            let source = hit.get("_source").cloned().unwrap_or(Value::Object(
                serde_json::Map::new(),
            ));

            let content = source
                .get(&self.config.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let metadata: HashMap<String, Value> = source
                .get("metadata")
                .and_then(|v| v.as_object())
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let doc = Document::with_metadata(id, content, metadata);
            results.push((doc, score));
        }

        Ok(results)
    }
}

/// Generate a simple unique ID.
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("{:x}-{:x}", timestamp, count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_defaults() {
        let config = ElasticsearchConfig::new("my_index", 1536);
        assert_eq!(config.index_name, "my_index");
        assert_eq!(config.dims, 1536);
        assert_eq!(config.url, "http://localhost:9200");
        assert_eq!(config.vector_field, "embedding");
        assert_eq!(config.content_field, "content");
        assert!(config.username.is_none());
        assert!(config.password.is_none());
    }

    #[test]
    fn config_with_url() {
        let config = ElasticsearchConfig::new("idx", 768)
            .with_url("https://es.example.com:9200");
        assert_eq!(config.url, "https://es.example.com:9200");
    }

    #[test]
    fn config_with_vector_field() {
        let config = ElasticsearchConfig::new("idx", 768)
            .with_vector_field("vec");
        assert_eq!(config.vector_field, "vec");
    }

    #[test]
    fn config_with_content_field() {
        let config = ElasticsearchConfig::new("idx", 768)
            .with_content_field("text");
        assert_eq!(config.content_field, "text");
    }

    #[test]
    fn config_with_auth() {
        let config = ElasticsearchConfig::new("idx", 768)
            .with_auth("elastic", "secret123");
        assert_eq!(config.username.as_deref(), Some("elastic"));
        assert_eq!(config.password.as_deref(), Some("secret123"));
    }

    #[test]
    fn config_builder_chain() {
        let config = ElasticsearchConfig::new("documents", 1536)
            .with_url("https://es-cluster:9200")
            .with_vector_field("doc_embedding")
            .with_content_field("doc_text")
            .with_auth("admin", "password");

        assert_eq!(config.index_name, "documents");
        assert_eq!(config.dims, 1536);
        assert_eq!(config.url, "https://es-cluster:9200");
        assert_eq!(config.vector_field, "doc_embedding");
        assert_eq!(config.content_field, "doc_text");
        assert_eq!(config.username.as_deref(), Some("admin"));
        assert_eq!(config.password.as_deref(), Some("password"));
    }

    #[test]
    fn store_new_creates_instance() {
        let config = ElasticsearchConfig::new("test_idx", 768);
        let store = ElasticsearchVectorStore::new(config);
        assert_eq!(store.config().index_name, "test_idx");
        assert_eq!(store.config().dims, 768);
    }

    #[test]
    fn url_construction() {
        let config = ElasticsearchConfig::new("idx", 768);
        let store = ElasticsearchVectorStore::new(config);
        assert_eq!(store.url("/_bulk"), "http://localhost:9200/_bulk");
        assert_eq!(store.url("/idx/_search"), "http://localhost:9200/idx/_search");
    }

    #[test]
    fn url_construction_trailing_slash() {
        let config = ElasticsearchConfig::new("idx", 768)
            .with_url("http://localhost:9200/");
        let store = ElasticsearchVectorStore::new(config);
        assert_eq!(store.url("/_bulk"), "http://localhost:9200/_bulk");
    }

    #[test]
    fn generate_id_is_unique() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn generate_id_is_non_empty() {
        let id = generate_id();
        assert!(!id.is_empty());
    }
}
