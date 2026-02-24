use std::collections::HashMap;
use synaptic_core::{Document, Embeddings, VectorStore};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_sqlite::{SqliteVectorStore, SqliteVectorStoreConfig};

fn make_doc(id: &str, content: &str) -> Document {
    Document {
        id: id.to_string(),
        content: content.to_string(),
        metadata: HashMap::new(),
    }
}

#[test]
fn config_builder() {
    let config = SqliteVectorStoreConfig::new("/tmp/vectors.db");
    assert_eq!(config.path, "/tmp/vectors.db");
}

#[test]
fn config_in_memory() {
    let config = SqliteVectorStoreConfig::in_memory();
    assert_eq!(config.path, ":memory:");
}

#[tokio::test]
async fn add_and_similarity_search() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let docs = vec![
        make_doc("1", "The cat sat on the mat"),
        make_doc("2", "The dog ran in the park"),
        make_doc("3", "Quantum physics is fascinating"),
    ];

    let ids = store.add_documents(docs, &emb).await.unwrap();
    assert_eq!(ids.len(), 3);

    let results = store.similarity_search("cat", 2, &emb).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn similarity_search_with_score() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let docs = vec![make_doc("1", "hello world"), make_doc("2", "goodbye world")];
    store.add_documents(docs, &emb).await.unwrap();

    let results = store
        .similarity_search_with_score("hello world", 2, &emb)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    // Scores should be finite
    for (_, score) in &results {
        assert!(score.is_finite(), "score should be finite, got {score}");
    }

    // Results should be sorted descending by score
    assert!(results[0].1 >= results[1].1);
}

#[tokio::test]
async fn similarity_search_by_vector() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let docs = vec![
        make_doc("1", "first document"),
        make_doc("2", "second document"),
    ];
    store.add_documents(docs, &emb).await.unwrap();

    // Use a query vector (from FakeEmbeddings, dimension-dependent)
    let query_vec = emb.embed_query("first").await.unwrap();
    let results = store
        .similarity_search_by_vector(&query_vec, 2)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn delete_documents() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let docs = vec![make_doc("1", "keep this"), make_doc("2", "delete this")];
    store.add_documents(docs, &emb).await.unwrap();

    store.delete(&["2"]).await.unwrap();

    let results = store.similarity_search("anything", 10, &emb).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "1");
}

#[tokio::test]
async fn empty_store_search() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let results = store.similarity_search("query", 5, &emb).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn metadata_preserved() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), serde_json::json!("test"));
    metadata.insert("page".to_string(), serde_json::json!(42));

    let doc = Document {
        id: "meta-doc".to_string(),
        content: "document with metadata".to_string(),
        metadata,
    };

    store.add_documents(vec![doc], &emb).await.unwrap();

    let results = store.similarity_search("document", 1, &emb).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].metadata.get("source"),
        Some(&serde_json::json!("test"))
    );
    assert_eq!(
        results[0].metadata.get("page"),
        Some(&serde_json::json!(42))
    );
}

#[tokio::test]
async fn empty_id_auto_uuid() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let doc = Document {
        id: String::new(),
        content: "auto id document".to_string(),
        metadata: HashMap::new(),
    };

    let ids = store.add_documents(vec![doc], &emb).await.unwrap();
    assert_eq!(ids.len(), 1);
    assert!(!ids[0].is_empty(), "should have auto-generated UUID");

    // Should be retrievable
    let results = store.similarity_search("auto", 1, &emb).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, ids[0]);
}

#[tokio::test]
async fn hybrid_search_basic() {
    let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory()).unwrap();
    let emb = FakeEmbeddings::new(16);

    let docs = vec![
        make_doc("1", "Rust programming language systems"),
        make_doc("2", "Python scripting language dynamic"),
        make_doc("3", "JavaScript browser web frontend"),
    ];
    store.add_documents(docs, &emb).await.unwrap();

    // alpha=0.5 balances vector and text similarity
    let results = store
        .hybrid_search("Rust programming", 2, &emb, 0.5)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results.len() <= 2);

    // All scores should be finite
    for (_, score) in &results {
        assert!(score.is_finite());
    }
}
