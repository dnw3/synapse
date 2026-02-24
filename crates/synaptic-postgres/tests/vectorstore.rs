//! Integration tests for `PgVectorStore`.
//!
//! The `#[ignore]` tests require a running PostgreSQL instance with the
//! pgvector extension installed. Set the `DATABASE_URL` environment variable
//! to the connection string before running:
//!
//! ```bash
//! DATABASE_URL=postgres://user:pass@localhost/test_db cargo test -p synaptic-postgres -- --ignored
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_postgres::{Document, Embeddings, PgVectorConfig, PgVectorStore, VectorStore};

use synaptic_core::SynapticError;

// ---------------------------------------------------------------------------
// Fake embeddings for integration tests
// ---------------------------------------------------------------------------

struct FakeEmbeddings {
    dimensions: usize,
}

impl FakeEmbeddings {
    fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait]
impl Embeddings for FakeEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        Ok(texts
            .iter()
            .map(|t| deterministic_vector(t, self.dimensions))
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        Ok(deterministic_vector(text, self.dimensions))
    }
}

/// Produce a deterministic embedding vector from text.
/// Uses a simple hash-based approach so identical texts yield identical vectors.
fn deterministic_vector(text: &str, dims: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dims];
    for (i, byte) in text.bytes().enumerate() {
        vec[i % dims] += byte as f32 / 255.0;
    }
    // Normalise to unit length for cosine similarity to behave sensibly.
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

// ---------------------------------------------------------------------------
// Unit tests (no database required)
// ---------------------------------------------------------------------------

#[test]
fn config_new() {
    let config = PgVectorConfig::new("test_table", 768);
    assert_eq!(config.table_name, "test_table");
    assert_eq!(config.vector_dimensions, 768);
}

#[test]
fn config_schema_qualified() {
    let config = PgVectorConfig::new("myschema.embeddings", 1536);
    assert_eq!(config.table_name, "myschema.embeddings");
}

// ---------------------------------------------------------------------------
// Integration tests (require a live PostgreSQL + pgvector instance)
// ---------------------------------------------------------------------------

/// Helper: create a store connected to the test database and initialise it.
async fn setup_store(table_name: &str, dims: u32) -> PgVectorStore {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for pgvector tests");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    // Drop the table first so each test starts fresh.
    let drop_sql = format!("DROP TABLE IF EXISTS {table_name}");
    sqlx::query(&drop_sql)
        .execute(&pool)
        .await
        .expect("failed to drop test table");

    let config = PgVectorConfig::new(table_name, dims);
    let store = PgVectorStore::new(pool, config);
    store.initialize().await.expect("initialize failed");
    store
}

#[tokio::test]
#[ignore]
async fn test_add_and_search_documents() {
    let dims: u32 = 64;
    let store = setup_store("test_add_search", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let docs = vec![
        Document::new("doc1", "Rust is a systems programming language"),
        Document::new("doc2", "Python is great for data science"),
        Document::new("doc3", "Rust has fearless concurrency"),
    ];

    let ids = store.add_documents(docs, &embeddings).await.unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], "doc1");

    // Search for Rust-related content.
    let results = store
        .similarity_search("Rust programming", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    // The top result should be about Rust.
    assert!(
        results[0].content.contains("Rust"),
        "expected Rust in top result, got: {}",
        results[0].content
    );
}

#[tokio::test]
#[ignore]
async fn test_similarity_search_with_score() {
    let dims: u32 = 64;
    let store = setup_store("test_search_score", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let docs = vec![
        Document::new("a", "hello world"),
        Document::new("b", "goodbye world"),
    ];
    store.add_documents(docs, &embeddings).await.unwrap();

    let results = store
        .similarity_search_with_score("hello world", 2, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    // The exact query text should have the highest score (cosine similarity = 1.0).
    let (top_doc, top_score) = &results[0];
    assert_eq!(top_doc.id, "a");
    assert!(
        *top_score > 0.99,
        "expected near-1.0 score for exact match, got {top_score}"
    );
}

#[tokio::test]
#[ignore]
async fn test_similarity_search_by_vector() {
    let dims: u32 = 64;
    let store = setup_store("test_search_vec", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let docs = vec![
        Document::new("x", "alpha beta gamma"),
        Document::new("y", "delta epsilon zeta"),
    ];
    store.add_documents(docs, &embeddings).await.unwrap();

    let query_vec = embeddings.embed_query("alpha beta gamma").await.unwrap();
    let results = store
        .similarity_search_by_vector(&query_vec, 1)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "x");
}

#[tokio::test]
#[ignore]
async fn test_delete_documents() {
    let dims: u32 = 64;
    let store = setup_store("test_delete", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let docs = vec![
        Document::new("d1", "keep this"),
        Document::new("d2", "delete this"),
        Document::new("d3", "also keep"),
    ];
    store.add_documents(docs, &embeddings).await.unwrap();

    store.delete(&["d2"]).await.unwrap();

    let results = store
        .similarity_search("delete this", 10, &embeddings)
        .await
        .unwrap();

    // d2 should be gone.
    assert!(
        results.iter().all(|d| d.id != "d2"),
        "deleted document should not appear in results"
    );
    assert_eq!(results.len(), 2);
}

#[tokio::test]
#[ignore]
async fn test_upsert_on_conflict() {
    let dims: u32 = 64;
    let store = setup_store("test_upsert", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let docs = vec![Document::new("u1", "original content")];
    store.add_documents(docs, &embeddings).await.unwrap();

    // Insert again with same id but different content.
    let docs = vec![Document::new("u1", "updated content")];
    store.add_documents(docs, &embeddings).await.unwrap();

    let results = store
        .similarity_search("updated content", 1, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "u1");
    assert_eq!(results[0].content, "updated content");
}

#[tokio::test]
#[ignore]
async fn test_auto_generated_ids() {
    let dims: u32 = 64;
    let store = setup_store("test_autoid", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    // Documents with empty ids should get auto-generated UUIDs.
    let docs = vec![
        Document::new("", "auto id document one"),
        Document::new("", "auto id document two"),
    ];
    let ids = store.add_documents(docs, &embeddings).await.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(!ids[0].is_empty());
    assert!(!ids[1].is_empty());
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
#[ignore]
async fn test_metadata_round_trip() {
    let dims: u32 = 64;
    let store = setup_store("test_metadata", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), Value::String("test".to_string()));
    metadata.insert("page".to_string(), Value::Number(42.into()));

    let doc = Document {
        id: "m1".to_string(),
        content: "metadata test".to_string(),
        metadata,
    };
    store.add_documents(vec![doc], &embeddings).await.unwrap();

    let results = store
        .similarity_search("metadata test", 1, &embeddings)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metadata.get("source").unwrap(), "test");
    assert_eq!(results[0].metadata.get("page").unwrap(), 42);
}

#[tokio::test]
#[ignore]
async fn test_empty_operations() {
    let dims: u32 = 64;
    let store = setup_store("test_empty_ops", dims).await;
    let embeddings = FakeEmbeddings::new(dims as usize);

    // Adding empty vec should succeed.
    let ids = store.add_documents(vec![], &embeddings).await.unwrap();
    assert!(ids.is_empty());

    // Deleting empty slice should succeed.
    store.delete(&[]).await.unwrap();

    // Searching an empty table should return empty results.
    let results = store
        .similarity_search("anything", 5, &embeddings)
        .await
        .unwrap();
    assert!(results.is_empty());
}
