use synaptic_mongodb::MongoVectorConfig;

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
        .with_index_name("custom_idx");
    assert_eq!(config.index_name, "custom_idx");
}

#[test]
fn config_with_vector_field() {
    let config = MongoVectorConfig::new("db", "col")
        .with_vector_field("vectors");
    assert_eq!(config.vector_field, "vectors");
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
        .with_num_candidates(300);
    assert_eq!(config.num_candidates, Some(300));
}

#[test]
fn config_builder_chain() {
    let config = MongoVectorConfig::new("test_db", "embeddings")
        .with_index_name("my_index")
        .with_vector_field("vec")
        .with_content_field("txt")
        .with_num_candidates(500);

    assert_eq!(config.database, "test_db");
    assert_eq!(config.collection, "embeddings");
    assert_eq!(config.index_name, "my_index");
    assert_eq!(config.vector_field, "vec");
    assert_eq!(config.content_field, "txt");
    assert_eq!(config.num_candidates, Some(500));
}

// ---------------------------------------------------------------------------
// Integration tests — require a running MongoDB Atlas instance with vector search.
// Run with: MONGODB_URI=... cargo test -p synaptic-mongodb -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
    use synaptic_mongodb::{MongoVectorConfig, MongoVectorStore};

    /// Simple fake embeddings for integration testing.
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

    async fn setup_store(collection: &str) -> MongoVectorStore {
        let uri = std::env::var("MONGODB_URI")
            .expect("MONGODB_URI must be set for MongoDB integration tests");
        let config = MongoVectorConfig::new("synaptic_test", collection);
        MongoVectorStore::from_uri(&uri, config)
            .await
            .expect("failed to connect to MongoDB")
    }

    #[tokio::test]
    #[ignore = "requires a running MongoDB Atlas instance with vector search"]
    async fn add_and_search_documents() {
        let store = setup_store("test_add_search").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("mg-1", "The quick brown fox jumps over the lazy dog"),
            Document::new("mg-2", "A fast red car drives down the highway"),
            Document::new("mg-3", "The lazy dog sleeps in the sun"),
        ];

        let ids = store.add_documents(docs, &embeddings).await.unwrap();
        assert_eq!(ids.len(), 3);

        // Wait for indexing.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let results = store
            .similarity_search("lazy dog sleeping", 2, &embeddings)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    #[ignore = "requires a running MongoDB Atlas instance with vector search"]
    async fn search_with_scores() {
        let store = setup_store("test_search_scores").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("ms-1", "Rust programming language"),
            Document::new("ms-2", "Python programming language"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let results = store
            .similarity_search_with_score("Rust language", 2, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].1 >= results[1].1);
    }

    #[tokio::test]
    #[ignore = "requires a running MongoDB Atlas instance with vector search"]
    async fn delete_documents() {
        let store = setup_store("test_delete").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("md-1", "first document"),
            Document::new("md-2", "second document"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        store.delete(&["md-2"]).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running MongoDB Atlas instance with vector search"]
    async fn metadata_is_preserved() {
        let store = setup_store("test_metadata").await;
        let embeddings = FakeEmbeddings::new(64);

        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::Value::String("test".into()));
        meta.insert("page".to_string(), serde_json::json!(42));

        let docs = vec![Document::with_metadata("mm-1", "metadata test", meta)];
        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let results = store
            .similarity_search("metadata test", 1, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].metadata.get("source"),
            Some(&serde_json::Value::String("test".into()))
        );
    }

    #[tokio::test]
    #[ignore = "requires a running MongoDB Atlas instance with vector search"]
    async fn empty_operations() {
        let store = setup_store("test_empty_ops").await;
        let embeddings = FakeEmbeddings::new(64);

        let ids = store.add_documents(vec![], &embeddings).await.unwrap();
        assert!(ids.is_empty());

        store.delete(&[]).await.unwrap();
    }
}
