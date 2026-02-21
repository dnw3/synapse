use std::collections::HashMap;

use async_trait::async_trait;
use qdrant_client::qdrant::{
    value::Kind, CreateCollectionBuilder, DeletePointsBuilder, Distance, PointId, PointStruct,
    PointsIdsList, ScoredPoint, SearchPointsBuilder, UpsertPointsBuilder, Value as QdrantValue,
    VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use serde_json::Value as JsonValue;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

// ---------------------------------------------------------------------------
// QdrantConfig
// ---------------------------------------------------------------------------

/// Configuration for connecting to a Qdrant instance.
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    /// Qdrant gRPC URL (e.g. `http://localhost:6334`).
    pub url: String,
    /// Name of the collection to operate on.
    pub collection_name: String,
    /// Dimensionality of the embedding vectors.
    pub vector_size: u64,
    /// Optional API key for authentication.
    pub api_key: Option<String>,
    /// Distance metric for similarity search. Defaults to `Cosine`.
    pub distance: Distance,
}

impl QdrantConfig {
    /// Create a new config with the required parameters.
    pub fn new(
        url: impl Into<String>,
        collection_name: impl Into<String>,
        vector_size: u64,
    ) -> Self {
        Self {
            url: url.into(),
            collection_name: collection_name.into(),
            vector_size,
            api_key: None,
            distance: Distance::Cosine,
        }
    }

    /// Set the API key for authenticated access.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the distance metric (default: Cosine).
    pub fn with_distance(mut self, distance: Distance) -> Self {
        self.distance = distance;
        self
    }
}

// ---------------------------------------------------------------------------
// QdrantVectorStore
// ---------------------------------------------------------------------------

/// A [`VectorStore`] implementation backed by [Qdrant](https://qdrant.tech/).
///
/// Each document is stored as a Qdrant point with:
/// - **id**: UUID (generated or derived from `Document::id`)
/// - **vector**: the embedding computed by the supplied `Embeddings`
/// - **payload**: `content` (string) and `metadata` (JSON object) fields
pub struct QdrantVectorStore {
    client: Qdrant,
    config: QdrantConfig,
}

impl QdrantVectorStore {
    /// Create a new store, connecting to Qdrant at the configured URL.
    pub fn new(config: QdrantConfig) -> Result<Self, SynapticError> {
        let mut builder = Qdrant::from_url(&config.url);
        if let Some(ref api_key) = config.api_key {
            builder = builder.api_key(api_key.clone());
        }
        let client = builder.build().map_err(|e| {
            SynapticError::VectorStore(format!("failed to build Qdrant client: {e}"))
        })?;
        Ok(Self { client, config })
    }

    /// Create a store from an existing [`Qdrant`] client.
    pub fn from_client(client: Qdrant, config: QdrantConfig) -> Self {
        Self { client, config }
    }

    /// Ensure the configured collection exists, creating it if necessary.
    pub async fn ensure_collection(&self) -> Result<(), SynapticError> {
        let exists = self
            .client
            .collection_exists(&self.config.collection_name)
            .await
            .map_err(|e| {
                SynapticError::VectorStore(format!("collection_exists check failed: {e}"))
            })?;

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.config.collection_name).vectors_config(
                        VectorParamsBuilder::new(self.config.vector_size, self.config.distance),
                    ),
                )
                .await
                .map_err(|e| {
                    SynapticError::VectorStore(format!("failed to create collection: {e}"))
                })?;
        }
        Ok(())
    }

    /// Return a reference to the underlying Qdrant client.
    pub fn client(&self) -> &Qdrant {
        &self.client
    }

    /// Return a reference to the config.
    pub fn config(&self) -> &QdrantConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for QdrantVectorStore {
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
        let mut points = Vec::with_capacity(docs.len());

        for (doc, vector) in docs.into_iter().zip(vectors) {
            // Use the document ID if non-empty, otherwise generate a UUID.
            let uuid_str = if doc.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                // If the ID is already a valid UUID, use it directly.
                // Otherwise, generate a deterministic UUID v5 from the ID.
                match uuid::Uuid::parse_str(&doc.id) {
                    Ok(_) => doc.id.clone(),
                    Err(_) => uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, doc.id.as_bytes())
                        .to_string(),
                }
            };

            // Build payload with `content` and `metadata` fields.
            let payload: HashMap<String, QdrantValue> = HashMap::from([
                (
                    "content".to_string(),
                    json_to_qdrant(&JsonValue::String(doc.content)),
                ),
                (
                    "metadata".to_string(),
                    json_to_qdrant(&JsonValue::Object(doc.metadata.into_iter().collect())),
                ),
                (
                    "doc_id".to_string(),
                    json_to_qdrant(&JsonValue::String(doc.id.clone())),
                ),
            ]);

            let point = PointStruct::new(uuid_str.clone(), vector, payload);
            ids.push(uuid_str);
            points.push(point);
        }

        self.client
            .upsert_points(UpsertPointsBuilder::new(
                &self.config.collection_name,
                points,
            ))
            .await
            .map_err(|e| SynapticError::VectorStore(format!("upsert failed: {e}")))?;

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

        let point_ids: Vec<PointId> = ids.iter().map(|id| string_to_point_id(id)).collect();

        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.config.collection_name)
                    .points(PointsIdsList { ids: point_ids }),
            )
            .await
            .map_err(|e| SynapticError::VectorStore(format!("delete failed: {e}")))?;

        Ok(())
    }
}

impl QdrantVectorStore {
    /// Search by vector and return documents with scores.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let response = self
            .client
            .search_points(
                SearchPointsBuilder::new(
                    &self.config.collection_name,
                    embedding.to_vec(),
                    k as u64,
                )
                .with_payload(true),
            )
            .await
            .map_err(|e| SynapticError::VectorStore(format!("search failed: {e}")))?;

        let results = response
            .result
            .into_iter()
            .map(scored_point_to_document)
            .collect();

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert a `serde_json::Value` into a `qdrant_client::qdrant::Value`.
fn json_to_qdrant(json: &JsonValue) -> QdrantValue {
    let kind = match json {
        JsonValue::Null => Some(Kind::NullValue(0)),
        JsonValue::Bool(b) => Some(Kind::BoolValue(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Kind::IntegerValue(i))
            } else {
                Some(Kind::DoubleValue(n.as_f64().unwrap_or(0.0)))
            }
        }
        JsonValue::String(s) => Some(Kind::StringValue(s.clone())),
        JsonValue::Array(arr) => {
            let values: Vec<QdrantValue> = arr.iter().map(json_to_qdrant).collect();
            Some(Kind::ListValue(qdrant_client::qdrant::ListValue { values }))
        }
        JsonValue::Object(map) => {
            let fields: HashMap<String, QdrantValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), json_to_qdrant(v)))
                .collect();
            Some(Kind::StructValue(qdrant_client::qdrant::Struct { fields }))
        }
    };
    QdrantValue { kind }
}

/// Convert a `qdrant_client::qdrant::Value` into a `serde_json::Value`.
fn qdrant_to_json(val: &QdrantValue) -> JsonValue {
    match &val.kind {
        None | Some(Kind::NullValue(_)) => JsonValue::Null,
        Some(Kind::BoolValue(b)) => JsonValue::Bool(*b),
        Some(Kind::IntegerValue(i)) => JsonValue::Number((*i).into()),
        Some(Kind::DoubleValue(d)) => serde_json::Number::from_f64(*d)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Some(Kind::StringValue(s)) => JsonValue::String(s.clone()),
        Some(Kind::ListValue(list)) => {
            JsonValue::Array(list.values.iter().map(qdrant_to_json).collect())
        }
        Some(Kind::StructValue(st)) => {
            let map: serde_json::Map<String, JsonValue> = st
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), qdrant_to_json(v)))
                .collect();
            JsonValue::Object(map)
        }
    }
}

/// Convert a string ID to a Qdrant [`PointId`].
///
/// If the string is a valid UUID, it is used as a UUID point ID.
/// Otherwise, a deterministic UUID v5 is generated from the string.
fn string_to_point_id(id: &str) -> PointId {
    let uuid_str = match uuid::Uuid::parse_str(id) {
        Ok(_) => id.to_string(),
        Err(_) => uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, id.as_bytes()).to_string(),
    };
    // PointId implements From<String>, which creates a UUID-based point ID.
    PointId::from(uuid_str)
}

/// Extract a [`Document`] and score from a Qdrant [`ScoredPoint`].
fn scored_point_to_document(sp: ScoredPoint) -> (Document, f32) {
    let score = sp.score;

    // Extract the original document ID from the `doc_id` payload field,
    // falling back to the point UUID.
    let point_uuid = sp
        .id
        .as_ref()
        .map(|pid| format!("{pid:?}"))
        .unwrap_or_default();

    let doc_id = sp
        .payload
        .get("doc_id")
        .and_then(|v| match &v.kind {
            Some(Kind::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or(point_uuid);

    let content = sp
        .payload
        .get("content")
        .and_then(|v| match &v.kind {
            Some(Kind::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let metadata: HashMap<String, JsonValue> = sp
        .payload
        .get("metadata")
        .map(|v| match qdrant_to_json(v) {
            JsonValue::Object(map) => map.into_iter().collect(),
            _ => HashMap::new(),
        })
        .unwrap_or_default();

    let doc = Document::with_metadata(doc_id, content, metadata);
    (doc, score)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_to_qdrant_roundtrip_string() {
        let json = JsonValue::String("hello".into());
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_number() {
        let json = serde_json::json!(42);
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_float() {
        let json = serde_json::json!(3.14);
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_bool() {
        let json = JsonValue::Bool(true);
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_null() {
        let json = JsonValue::Null;
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_array() {
        let json = serde_json::json!([1, "two", true]);
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_qdrant_roundtrip_object() {
        let json = serde_json::json!({"key": "value", "num": 42});
        let qdrant = json_to_qdrant(&json);
        let back = qdrant_to_json(&qdrant);
        assert_eq!(json, back);
    }

    #[test]
    fn string_to_point_id_with_valid_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let pid = string_to_point_id(uuid_str);
        assert!(pid.point_id_options.is_some());
    }

    #[test]
    fn string_to_point_id_with_non_uuid_is_deterministic() {
        let pid1 = string_to_point_id("my-doc-id");
        let pid2 = string_to_point_id("my-doc-id");
        assert_eq!(pid1, pid2);
    }

    #[test]
    fn string_to_point_id_different_ids_produce_different_points() {
        let pid1 = string_to_point_id("doc-1");
        let pid2 = string_to_point_id("doc-2");
        assert_ne!(pid1, pid2);
    }
}
