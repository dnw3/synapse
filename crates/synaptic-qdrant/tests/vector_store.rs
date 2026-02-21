use synaptic_qdrant::{QdrantConfig, QdrantVectorStore};

#[test]
fn config_new_sets_defaults() {
    let config = QdrantConfig::new("http://localhost:6334", "test_collection", 1536);
    assert_eq!(config.url, "http://localhost:6334");
    assert_eq!(config.collection_name, "test_collection");
    assert_eq!(config.vector_size, 1536);
    assert!(config.api_key.is_none());
}

#[test]
fn config_with_api_key() {
    let config = QdrantConfig::new("http://localhost:6334", "test_collection", 1536)
        .with_api_key("my-secret-key");
    assert_eq!(config.api_key.as_deref(), Some("my-secret-key"));
}

#[test]
fn config_with_distance() {
    use qdrant_client::qdrant::Distance;

    let config = QdrantConfig::new("http://localhost:6334", "test_collection", 1536)
        .with_distance(Distance::Euclid);
    assert_eq!(config.distance, Distance::Euclid);
}

#[test]
fn config_builder_chain() {
    let config = QdrantConfig::new("http://qdrant.example.com:6334", "embeddings", 768)
        .with_api_key("key123")
        .with_distance(qdrant_client::qdrant::Distance::Dot);

    assert_eq!(config.url, "http://qdrant.example.com:6334");
    assert_eq!(config.collection_name, "embeddings");
    assert_eq!(config.vector_size, 768);
    assert_eq!(config.api_key.as_deref(), Some("key123"));
    assert_eq!(config.distance, qdrant_client::qdrant::Distance::Dot);
}

#[test]
fn store_new_creates_client() {
    // This test verifies that the constructor successfully builds a client
    // without requiring a running Qdrant instance.
    let config = QdrantConfig::new("http://localhost:6334", "test_collection", 1536);
    let store = QdrantVectorStore::new(config);
    assert!(store.is_ok());
}

#[test]
fn store_new_with_api_key() {
    let config =
        QdrantConfig::new("http://localhost:6334", "test_collection", 1536).with_api_key("secret");
    let store = QdrantVectorStore::new(config);
    assert!(store.is_ok());
}

#[test]
fn store_config_accessor() {
    let config = QdrantConfig::new("http://localhost:6334", "my_col", 512);
    let store = QdrantVectorStore::new(config).unwrap();
    assert_eq!(store.config().collection_name, "my_col");
    assert_eq!(store.config().vector_size, 512);
}

// ---------------------------------------------------------------------------
// Integration tests — require a running Qdrant instance.
// Run with: cargo test -p synaptic-qdrant -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
    use synaptic_qdrant::{QdrantConfig, QdrantVectorStore};

    /// Simple fake embeddings for integration testing.
    /// Returns deterministic vectors based on the text hash.
    struct FakeEmbeddings {
        dim: usize,
    }

    impl FakeEmbeddings {
        fn new(dim: usize) -> Self {
            Self { dim }
        }

        fn embed_text(&self, text: &str) -> Vec<f32> {
            let mut vec = vec![0.0f32; self.dim];
            for (i, byte) in text.bytes().enumerate() {
                vec[i % self.dim] += byte as f32 / 255.0;
            }
            // Normalize to unit vector.
            let mag: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if mag > 0.0 {
                vec.iter_mut().for_each(|x| *x /= mag);
            }
            vec
        }
    }

    #[async_trait]
    impl Embeddings for FakeEmbeddings {
        async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
            Ok(texts.iter().map(|t| self.embed_text(t)).collect())
        }

        async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
            Ok(self.embed_text(text))
        }
    }

    /// Create a test store connected to a local Qdrant instance.
    async fn setup_store(collection: &str, dim: u64) -> QdrantVectorStore {
        let config = QdrantConfig::new("http://localhost:6334", collection, dim);
        let store = QdrantVectorStore::new(config).expect("failed to create store");

        // Clean up any previous test data by deleting and recreating the collection.
        let _ = store.client().delete_collection(collection).await;
        store
            .ensure_collection()
            .await
            .expect("failed to ensure collection");

        store
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn add_and_search_documents() {
        let dim = 64;
        let store = setup_store("test_add_search", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        let docs = vec![
            Document::new("doc-1", "The quick brown fox jumps over the lazy dog"),
            Document::new("doc-2", "A fast red car drives down the highway"),
            Document::new("doc-3", "The lazy dog sleeps in the sun"),
        ];

        let ids = store.add_documents(docs, &embeddings).await.unwrap();
        assert_eq!(ids.len(), 3);

        // Wait briefly for indexing.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Search for something similar to "lazy dog".
        let results = store
            .similarity_search("lazy dog sleeping", 2, &embeddings)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn search_with_scores() {
        let dim = 64;
        let store = setup_store("test_search_scores", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        let docs = vec![
            Document::new("s-1", "Rust programming language"),
            Document::new("s-2", "Python programming language"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let results = store
            .similarity_search_with_score("Rust language", 2, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // First result should have the highest score.
        assert!(results[0].1 >= results[1].1);
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn search_by_vector() {
        let dim = 64;
        let store = setup_store("test_search_vector", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        let docs = vec![Document::new("v-1", "hello world")];
        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let query_vec = embeddings.embed_text("hello world");
        let results = store
            .similarity_search_by_vector(&query_vec, 1)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello world");
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn delete_documents() {
        let dim = 64;
        let store = setup_store("test_delete", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        let docs = vec![
            Document::new("d-1", "first document"),
            Document::new("d-2", "second document"),
        ];

        let ids = store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Delete the first document.
        store.delete(&[ids[0].as_str()]).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Should only find one document now.
        let results = store
            .similarity_search("document", 10, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn metadata_is_preserved() {
        let dim = 64;
        let store = setup_store("test_metadata", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        let mut meta = HashMap::new();
        meta.insert(
            "source".to_string(),
            serde_json::Value::String("test".into()),
        );
        meta.insert("page".to_string(), serde_json::json!(42));

        let docs = vec![Document::with_metadata("m-1", "metadata test", meta)];
        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let results = store
            .similarity_search("metadata test", 1, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].metadata.get("source"),
            Some(&serde_json::Value::String("test".into()))
        );
        assert_eq!(
            results[0].metadata.get("page"),
            Some(&serde_json::json!(42))
        );
    }

    #[tokio::test]
    #[ignore = "requires running Qdrant instance at localhost:6334"]
    async fn empty_operations() {
        let dim = 64;
        let store = setup_store("test_empty_ops", dim as u64).await;
        let embeddings = FakeEmbeddings::new(dim);

        // Adding zero documents should succeed.
        let ids = store.add_documents(vec![], &embeddings).await.unwrap();
        assert!(ids.is_empty());

        // Deleting zero IDs should succeed.
        store.delete(&[]).await.unwrap();
    }
}
