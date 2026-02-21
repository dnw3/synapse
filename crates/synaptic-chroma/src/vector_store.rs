use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// ChromaConfig
// ---------------------------------------------------------------------------

/// Configuration for connecting to a ChromaDB instance.
#[derive(Debug, Clone)]
pub struct ChromaConfig {
    /// ChromaDB server URL (default: `http://localhost:8000`).
    pub url: String,
    /// Name of the collection to operate on.
    pub collection_name: String,
    /// Tenant name (default: `default_tenant`).
    pub tenant: String,
    /// Database name (default: `default_database`).
    pub database: String,
}

impl ChromaConfig {
    /// Create a new config with the required collection name.
    ///
    /// Uses default values for URL (`http://localhost:8000`), tenant
    /// (`default_tenant`), and database (`default_database`).
    pub fn new(collection_name: impl Into<String>) -> Self {
        Self {
            url: "http://localhost:8000".to_string(),
            collection_name: collection_name.into(),
            tenant: "default_tenant".to_string(),
            database: "default_database".to_string(),
        }
    }

    /// Set the ChromaDB server URL.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Set the tenant name.
    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = tenant.into();
        self
    }

    /// Set the database name.
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = database.into();
        self
    }
}

// ---------------------------------------------------------------------------
// ChromaVectorStore
// ---------------------------------------------------------------------------

/// A [`VectorStore`] implementation backed by [ChromaDB](https://www.trychroma.com/).
///
/// Uses the Chroma REST API v1 for all operations. Documents are stored
/// with their embeddings, content, and metadata in a named collection.
///
/// Call [`ensure_collection`](ChromaVectorStore::ensure_collection) before
/// performing any operations to create or retrieve the collection.
pub struct ChromaVectorStore {
    config: ChromaConfig,
    client: reqwest::Client,
    /// Cached collection ID, populated lazily by `ensure_collection`.
    collection_id: RwLock<Option<String>>,
}

impl ChromaVectorStore {
    /// Create a new store with the given configuration.
    pub fn new(config: ChromaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            collection_id: RwLock::new(None),
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &ChromaConfig {
        &self.config
    }

    /// Ensure the configured collection exists, creating it if necessary.
    ///
    /// The collection ID is cached after the first successful call.
    pub async fn ensure_collection(&self) -> Result<(), SynapticError> {
        // Check if already cached.
        {
            let cached = self.collection_id.read().await;
            if cached.is_some() {
                return Ok(());
            }
        }

        let url = format!(
            "{}/api/v1/tenants/{}/databases/{}/collections",
            self.config.url.trim_end_matches('/'),
            self.config.tenant,
            self.config.database,
        );

        let body = serde_json::json!({
            "name": self.config.collection_name,
            "get_or_create": true,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                SynapticError::VectorStore(format!("Chroma create collection failed: {e}"))
            })?;

        let status = response.status();
        let text = response.text().await.map_err(|e| {
            SynapticError::VectorStore(format!("failed to read Chroma response: {e}"))
        })?;

        if !status.is_success() {
            return Err(SynapticError::VectorStore(format!(
                "Chroma API error (HTTP {status}): {text}"
            )));
        }

        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            SynapticError::VectorStore(format!("failed to parse Chroma response: {e}"))
        })?;

        let id = parsed
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SynapticError::VectorStore("Chroma response missing collection id".to_string())
            })?
            .to_string();

        let mut cached = self.collection_id.write().await;
        *cached = Some(id);

        Ok(())
    }

    /// Get the cached collection ID, returning an error if not yet initialized.
    async fn get_collection_id(&self) -> Result<String, SynapticError> {
        let cached = self.collection_id.read().await;
        cached.clone().ok_or_else(|| {
            SynapticError::VectorStore(
                "collection not initialized; call ensure_collection() first".to_string(),
            )
        })
    }

    /// Build the collection-scoped API URL.
    async fn collection_url(&self, path: &str) -> Result<String, SynapticError> {
        let id = self.get_collection_id().await?;
        Ok(format!(
            "{}/api/v1/collections/{id}{path}",
            self.config.url.trim_end_matches('/'),
        ))
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for ChromaVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError> {
        if docs.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_collection().await?;

        // Compute embeddings for all documents.
        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let mut ids = Vec::with_capacity(docs.len());
        let mut documents = Vec::with_capacity(docs.len());
        let mut metadatas = Vec::with_capacity(docs.len());
        let mut emb_list = Vec::with_capacity(docs.len());

        for (doc, vector) in docs.into_iter().zip(vectors) {
            let id = if doc.id.is_empty() {
                uuid_v4()
            } else {
                doc.id.clone()
            };

            // Chroma metadata only accepts primitive types (string, int, float, bool).
            // We flatten the metadata, converting non-primitive values to JSON strings.
            let mut chroma_meta = serde_json::Map::new();
            for (k, v) in &doc.metadata {
                let flat = match v {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) => v.clone(),
                    _ => Value::String(v.to_string()),
                };
                chroma_meta.insert(k.clone(), flat);
            }

            ids.push(id);
            documents.push(doc.content);
            metadatas.push(Value::Object(chroma_meta));
            emb_list.push(vector);
        }

        let url = self.collection_url("/add").await?;
        let body = serde_json::json!({
            "ids": ids,
            "embeddings": emb_list,
            "documents": documents,
            "metadatas": metadatas,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Chroma add failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "Chroma add error (HTTP {status}): {text}"
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

        self.ensure_collection().await?;

        let url = self.collection_url("/delete").await?;
        let id_values: Vec<&str> = ids.to_vec();
        let body = serde_json::json!({
            "ids": id_values,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Chroma delete failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(SynapticError::VectorStore(format!(
                "Chroma delete error (HTTP {status}): {text}"
            )));
        }

        Ok(())
    }
}

impl ChromaVectorStore {
    /// Search by vector and return documents with similarity scores.
    ///
    /// Chroma returns distances (lower is better), which we convert to scores
    /// using `score = 1.0 / (1.0 + distance)`.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        self.ensure_collection().await?;

        let url = self.collection_url("/query").await?;
        let body = serde_json::json!({
            "query_embeddings": [embedding],
            "n_results": k,
            "include": ["documents", "metadatas", "distances"],
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::VectorStore(format!("Chroma query failed: {e}")))?;

        let status = response.status();
        let text = response.text().await.map_err(|e| {
            SynapticError::VectorStore(format!("failed to read Chroma response: {e}"))
        })?;

        if !status.is_success() {
            return Err(SynapticError::VectorStore(format!(
                "Chroma query error (HTTP {status}): {text}"
            )));
        }

        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            SynapticError::VectorStore(format!("failed to parse Chroma response: {e}"))
        })?;

        // Chroma query response structure (arrays of arrays, one per query):
        // { "ids": [[...]], "documents": [[...]], "metadatas": [[...]], "distances": [[...]] }
        let ids = parsed["ids"]
            .get(0)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let documents = parsed["documents"]
            .get(0)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let metadatas = parsed["metadatas"]
            .get(0)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let distances = parsed["distances"]
            .get(0)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut results = Vec::with_capacity(ids.len());

        for (i, id_val) in ids.iter().enumerate() {
            let id = id_val.as_str().unwrap_or("").to_string();
            let content = documents
                .get(i)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let metadata: HashMap<String, Value> = metadatas
                .get(i)
                .and_then(|v| v.as_object())
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let distance = distances
                .get(i)
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            // Convert distance to score (lower distance = higher score).
            let score = 1.0 / (1.0 + distance);

            let doc = Document::with_metadata(id, content, metadata);
            results.push((doc, score));
        }

        Ok(results)
    }
}

/// Generate a UUID v4 string without pulling in the uuid crate.
/// Uses a simple random approach via timestamp-based seed.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Simple pseudo-random UUID for document IDs.
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (nanos & 0xFFFF_FFFF) as u32,
        ((nanos >> 32) & 0xFFFF) as u16,
        ((nanos >> 48) & 0x0FFF) as u16,
        (0x8000 | ((nanos >> 60) & 0x3FFF)) as u16,
        ((nanos >> 74) ^ nanos) & 0xFFFF_FFFF_FFFF,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_defaults() {
        let config = ChromaConfig::new("my_collection");
        assert_eq!(config.collection_name, "my_collection");
        assert_eq!(config.url, "http://localhost:8000");
        assert_eq!(config.tenant, "default_tenant");
        assert_eq!(config.database, "default_database");
    }

    #[test]
    fn config_with_url() {
        let config = ChromaConfig::new("col").with_url("http://chroma:9000");
        assert_eq!(config.url, "http://chroma:9000");
    }

    #[test]
    fn config_with_tenant() {
        let config = ChromaConfig::new("col").with_tenant("my_tenant");
        assert_eq!(config.tenant, "my_tenant");
    }

    #[test]
    fn config_with_database() {
        let config = ChromaConfig::new("col").with_database("my_db");
        assert_eq!(config.database, "my_db");
    }

    #[test]
    fn config_builder_chain() {
        let config = ChromaConfig::new("embeddings")
            .with_url("http://chroma.example.com:8080")
            .with_tenant("acme")
            .with_database("production");

        assert_eq!(config.collection_name, "embeddings");
        assert_eq!(config.url, "http://chroma.example.com:8080");
        assert_eq!(config.tenant, "acme");
        assert_eq!(config.database, "production");
    }

    #[test]
    fn store_new_creates_instance() {
        let config = ChromaConfig::new("test_col");
        let store = ChromaVectorStore::new(config);
        assert_eq!(store.config().collection_name, "test_col");
    }

    #[test]
    fn uuid_v4_produces_non_empty_string() {
        let id = uuid_v4();
        assert!(!id.is_empty());
        // Should have the 4 dashes of a UUID.
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn distance_to_score_conversion() {
        // distance 0 => score 1.0
        let score = 1.0 / (1.0 + 0.0_f32);
        assert!((score - 1.0).abs() < f32::EPSILON);

        // distance 1 => score 0.5
        let score = 1.0 / (1.0 + 1.0_f32);
        assert!((score - 0.5).abs() < f32::EPSILON);

        // Higher distance => lower score.
        let s1 = 1.0 / (1.0 + 0.5_f32);
        let s2 = 1.0 / (1.0 + 2.0_f32);
        assert!(s1 > s2);
    }
}
