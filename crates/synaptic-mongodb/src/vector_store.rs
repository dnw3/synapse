use std::collections::HashMap;

use async_trait::async_trait;
use bson::{doc, Bson, Document as BsonDocument};
use futures::TryStreamExt;
use mongodb::Client;
use serde_json::Value;
use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};

// ---------------------------------------------------------------------------
// MongoVectorConfig
// ---------------------------------------------------------------------------

/// Configuration for a [`MongoVectorStore`].
#[derive(Debug, Clone)]
pub struct MongoVectorConfig {
    /// MongoDB database name.
    pub database: String,
    /// MongoDB collection name.
    pub collection: String,
    /// Name of the Atlas Vector Search index (default: `vector_index`).
    pub index_name: String,
    /// Field name storing the embedding vector (default: `embedding`).
    pub vector_field: String,
    /// Field name storing the document content (default: `content`).
    pub content_field: String,
    /// Number of candidates for `$vectorSearch` (default: `10 * k`).
    pub num_candidates: Option<i64>,
}

impl MongoVectorConfig {
    /// Create a new config with the required database and collection names.
    pub fn new(database: impl Into<String>, collection: impl Into<String>) -> Self {
        Self {
            database: database.into(),
            collection: collection.into(),
            index_name: "vector_index".to_string(),
            vector_field: "embedding".to_string(),
            content_field: "content".to_string(),
            num_candidates: None,
        }
    }

    /// Set the vector search index name.
    pub fn with_index_name(mut self, index_name: impl Into<String>) -> Self {
        self.index_name = index_name.into();
        self
    }

    /// Set the field name for storing embedding vectors.
    pub fn with_vector_field(mut self, vector_field: impl Into<String>) -> Self {
        self.vector_field = vector_field.into();
        self
    }

    /// Set the field name for storing document content.
    pub fn with_content_field(mut self, content_field: impl Into<String>) -> Self {
        self.content_field = content_field.into();
        self
    }

    /// Set the number of candidates for `$vectorSearch`.
    ///
    /// If not set, defaults to `10 * k` at query time.
    pub fn with_num_candidates(mut self, num_candidates: i64) -> Self {
        self.num_candidates = Some(num_candidates);
        self
    }
}

// ---------------------------------------------------------------------------
// MongoVectorStore
// ---------------------------------------------------------------------------

/// A [`VectorStore`] implementation backed by MongoDB Atlas Vector Search.
///
/// Documents are stored in a MongoDB collection with fields:
/// - `_id`: the document ID
/// - `content`: the document text
/// - `embedding`: the vector embedding (array of doubles)
/// - `metadata`: an embedded document with arbitrary metadata
///
/// Similarity search uses the `$vectorSearch` aggregation stage, which requires
/// a pre-configured Atlas Vector Search index on the collection.
pub struct MongoVectorStore {
    config: MongoVectorConfig,
    client: Client,
    collection: mongodb::Collection<BsonDocument>,
}

impl MongoVectorStore {
    /// Create a new store by connecting to MongoDB at the given URI.
    pub async fn from_uri(
        uri: &str,
        config: MongoVectorConfig,
    ) -> Result<Self, SynapticError> {
        let client = Client::with_uri_str(uri).await.map_err(|e| {
            SynapticError::VectorStore(format!("failed to connect to MongoDB: {e}"))
        })?;

        Ok(Self::from_client(client, config))
    }

    /// Create a new store from an existing MongoDB client.
    pub fn from_client(client: Client, config: MongoVectorConfig) -> Self {
        let db = client.database(&config.database);
        let collection = db.collection::<BsonDocument>(&config.collection);
        Self {
            config,
            client,
            collection,
        }
    }

    /// Return a reference to the underlying MongoDB client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &MongoVectorConfig {
        &self.config
    }

    /// Return a reference to the underlying MongoDB collection.
    pub fn collection(&self) -> &mongodb::Collection<BsonDocument> {
        &self.collection
    }

    /// Compute the number of candidates to use in `$vectorSearch`.
    fn num_candidates(&self, k: usize) -> i64 {
        self.config
            .num_candidates
            .unwrap_or_else(|| (k as i64) * 10)
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for MongoVectorStore {
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
        let mut bson_docs = Vec::with_capacity(docs.len());

        for (doc, vector) in docs.into_iter().zip(vectors) {
            let id = if doc.id.is_empty() {
                bson::oid::ObjectId::new().to_hex()
            } else {
                doc.id.clone()
            };

            // Convert the embedding vector to BSON array of doubles.
            let bson_vector: Vec<Bson> = vector.into_iter().map(|v| Bson::Double(v as f64)).collect();

            // Convert metadata to BSON document.
            let metadata_bson = json_map_to_bson(&doc.metadata);

            let bson_doc = doc! {
                "_id": &id,
                &self.config.content_field: &doc.content,
                &self.config.vector_field: bson_vector,
                "metadata": metadata_bson,
            };

            ids.push(id);
            bson_docs.push(bson_doc);
        }

        self.collection
            .insert_many(bson_docs)
            .await
            .map_err(|e| SynapticError::VectorStore(format!("MongoDB insert failed: {e}")))?;

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

        let id_values: Vec<Bson> = ids.iter().map(|id| Bson::String(id.to_string())).collect();

        self.collection
            .delete_many(doc! { "_id": { "$in": id_values } })
            .await
            .map_err(|e| SynapticError::VectorStore(format!("MongoDB delete failed: {e}")))?;

        Ok(())
    }
}

impl MongoVectorStore {
    /// Search by vector and return documents with their similarity scores.
    ///
    /// Uses the `$vectorSearch` aggregation pipeline stage available in
    /// MongoDB Atlas.
    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>, SynapticError> {
        let num_candidates = self.num_candidates(k);

        // Convert embedding to BSON array.
        let query_vector: Vec<Bson> = embedding.iter().map(|v| Bson::Double(*v as f64)).collect();

        // Build the $vectorSearch stage.
        let vector_search_stage = doc! {
            "$vectorSearch": {
                "index": &self.config.index_name,
                "path": &self.config.vector_field,
                "queryVector": query_vector,
                "numCandidates": num_candidates,
                "limit": k as i64,
            }
        };

        // Build the $project stage to include the score.
        let project_stage = doc! {
            "$project": {
                "_id": 1,
                &self.config.content_field: 1,
                "metadata": 1,
                "score": { "$meta": "vectorSearchScore" },
            }
        };

        let pipeline = vec![vector_search_stage, project_stage];

        let mut cursor = self
            .collection
            .aggregate(pipeline)
            .await
            .map_err(|e| SynapticError::VectorStore(format!("MongoDB aggregation failed: {e}")))?;

        let mut results = Vec::new();

        while let Some(bson_doc) = cursor.try_next().await.map_err(|e| {
            SynapticError::VectorStore(format!("MongoDB cursor error: {e}"))
        })? {
            let id = bson_doc
                .get_str("_id")
                .unwrap_or("")
                .to_string();

            let content = bson_doc
                .get_str(&self.config.content_field)
                .unwrap_or("")
                .to_string();

            let score = bson_doc
                .get_f64("score")
                .unwrap_or(0.0) as f32;

            let metadata = bson_doc
                .get_document("metadata")
                .ok()
                .map(bson_doc_to_json_map)
                .unwrap_or_default();

            let doc = Document::with_metadata(id, content, metadata);
            results.push((doc, score));
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert a JSON metadata map to a BSON document.
fn json_map_to_bson(map: &HashMap<String, Value>) -> BsonDocument {
    let mut doc = BsonDocument::new();
    for (k, v) in map {
        doc.insert(k.clone(), json_to_bson(v));
    }
    doc
}

/// Convert a `serde_json::Value` to a `bson::Bson` value.
fn json_to_bson(value: &Value) -> Bson {
    match value {
        Value::Null => Bson::Null,
        Value::Bool(b) => Bson::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Bson::Int64(i)
            } else if let Some(f) = n.as_f64() {
                Bson::Double(f)
            } else {
                Bson::Null
            }
        }
        Value::String(s) => Bson::String(s.clone()),
        Value::Array(arr) => {
            Bson::Array(arr.iter().map(json_to_bson).collect())
        }
        Value::Object(map) => {
            let mut doc = BsonDocument::new();
            for (k, v) in map {
                doc.insert(k.clone(), json_to_bson(v));
            }
            Bson::Document(doc)
        }
    }
}

/// Convert a BSON document to a JSON metadata map.
fn bson_doc_to_json_map(doc: &BsonDocument) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    for (k, v) in doc {
        map.insert(k.clone(), bson_to_json(v));
    }
    map
}

/// Convert a `bson::Bson` value to a `serde_json::Value`.
fn bson_to_json(bson: &Bson) -> Value {
    match bson {
        Bson::Null => Value::Null,
        Bson::Boolean(b) => Value::Bool(*b),
        Bson::Int32(i) => Value::Number((*i as i64).into()),
        Bson::Int64(i) => Value::Number((*i).into()),
        Bson::Double(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Bson::String(s) => Value::String(s.clone()),
        Bson::Array(arr) => Value::Array(arr.iter().map(bson_to_json).collect()),
        Bson::Document(doc) => {
            let map: serde_json::Map<String, Value> = doc
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            Value::Object(map)
        }
        Bson::ObjectId(oid) => Value::String(oid.to_hex()),
        Bson::DateTime(dt) => Value::String(dt.to_string()),
        Bson::Binary(bin) => Value::String(format!("<binary {} bytes>", bin.bytes.len())),
        _ => Value::String(format!("{bson}")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_sets_defaults() {
        let config = MongoVectorConfig::new("my_db", "my_collection");
        assert_eq!(config.database, "my_db");
        assert_eq!(config.collection, "my_collection");
        assert_eq!(config.index_name, "vector_index");
        assert_eq!(config.vector_field, "embedding");
        assert_eq!(config.content_field, "content");
        assert!(config.num_candidates.is_none());
    }

    #[test]
    fn config_with_index_name() {
        let config = MongoVectorConfig::new("db", "col")
            .with_index_name("custom_index");
        assert_eq!(config.index_name, "custom_index");
    }

    #[test]
    fn config_with_vector_field() {
        let config = MongoVectorConfig::new("db", "col")
            .with_vector_field("vec");
        assert_eq!(config.vector_field, "vec");
    }

    #[test]
    fn config_with_content_field() {
        let config = MongoVectorConfig::new("db", "col")
            .with_content_field("text");
        assert_eq!(config.content_field, "text");
    }

    #[test]
    fn config_with_num_candidates() {
        let config = MongoVectorConfig::new("db", "col")
            .with_num_candidates(200);
        assert_eq!(config.num_candidates, Some(200));
    }

    #[test]
    fn config_builder_chain() {
        let config = MongoVectorConfig::new("test_db", "embeddings")
            .with_index_name("my_vs_index")
            .with_vector_field("vec_field")
            .with_content_field("text_field")
            .with_num_candidates(500);

        assert_eq!(config.database, "test_db");
        assert_eq!(config.collection, "embeddings");
        assert_eq!(config.index_name, "my_vs_index");
        assert_eq!(config.vector_field, "vec_field");
        assert_eq!(config.content_field, "text_field");
        assert_eq!(config.num_candidates, Some(500));
    }

    #[test]
    fn json_to_bson_roundtrip_string() {
        let json = Value::String("hello".into());
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_number_int() {
        let json = serde_json::json!(42);
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_number_float() {
        let json = serde_json::json!(3.14);
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_bool() {
        let json = Value::Bool(true);
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_null() {
        let json = Value::Null;
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_array() {
        let json = serde_json::json!([1, "two", true]);
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_to_bson_roundtrip_object() {
        let json = serde_json::json!({"key": "value", "num": 42});
        let bson = json_to_bson(&json);
        let back = bson_to_json(&bson);
        assert_eq!(json, back);
    }

    #[test]
    fn json_map_to_bson_and_back() {
        let mut map = HashMap::new();
        map.insert("source".to_string(), Value::String("test".into()));
        map.insert("page".to_string(), serde_json::json!(42));

        let bson_doc = json_map_to_bson(&map);
        let back = bson_doc_to_json_map(&bson_doc);

        assert_eq!(map, back);
    }

    #[test]
    fn num_candidates_default() {
        let config = MongoVectorConfig::new("db", "col");
        // We cannot call num_candidates() without a MongoVectorStore, but we can
        // test the logic directly.
        let k = 10_usize;
        let result = config.num_candidates.unwrap_or_else(|| (k as i64) * 10);
        assert_eq!(result, 100);
    }

    #[test]
    fn num_candidates_custom() {
        let config = MongoVectorConfig::new("db", "col")
            .with_num_candidates(200);
        let k = 10_usize;
        let result = config.num_candidates.unwrap_or_else(|| (k as i64) * 10);
        assert_eq!(result, 200);
    }
}
