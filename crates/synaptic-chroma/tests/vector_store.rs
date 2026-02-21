use synaptic_chroma::{ChromaConfig, ChromaVectorStore};

#[test]
fn config_new_sets_defaults() {
    let config = ChromaConfig::new("test_collection");
    assert_eq!(config.collection_name, "test_collection");
    assert_eq!(config.url, "http://localhost:8000");
    assert_eq!(config.tenant, "default_tenant");
    assert_eq!(config.database, "default_database");
}

#[test]
fn config_with_url() {
    let config = ChromaConfig::new("col").with_url("http://remote:9000");
    assert_eq!(config.url, "http://remote:9000");
}

#[test]
fn config_with_tenant_and_database() {
    let config = ChromaConfig::new("col")
        .with_tenant("my_tenant")
        .with_database("my_db");
    assert_eq!(config.tenant, "my_tenant");
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
fn store_config_accessor() {
    let config = ChromaConfig::new("my_embeddings")
        .with_url("http://chroma:8000")
        .with_tenant("test_tenant");
    let store = ChromaVectorStore::new(config);
    assert_eq!(store.config().url, "http://chroma:8000");
    assert_eq!(store.config().tenant, "test_tenant");
}

// ---------------------------------------------------------------------------
// Integration tests — require a running ChromaDB instance.
// Run with: cargo test -p synaptic-chroma -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
    use synaptic_chroma::{ChromaConfig, ChromaVectorStore};

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

    async fn setup_store(collection: &str) -> ChromaVectorStore {
        let url = std::env::var("CHROMA_URL").unwrap_or_else(|_| "http://localhost:8000".into());
        let config = ChromaConfig::new(collection).with_url(url);
        let store = ChromaVectorStore::new(config);
        store.ensure_collection().await.expect("failed to ensure collection");
        store
    }

    #[tokio::test]
    #[ignore = "requires running ChromaDB instance"]
    async fn add_and_search_documents() {
        let store = setup_store("test_add_search").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("ch-1", "The quick brown fox jumps over the lazy dog"),
            Document::new("ch-2", "A fast red car drives down the highway"),
            Document::new("ch-3", "The lazy dog sleeps in the sun"),
        ];

        let ids = store.add_documents(docs, &embeddings).await.unwrap();
        assert_eq!(ids.len(), 3);

        let results = store
            .similarity_search("lazy dog sleeping", 2, &embeddings)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    #[ignore = "requires running ChromaDB instance"]
    async fn search_with_scores() {
        let store = setup_store("test_search_scores").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("cs-1", "Rust programming language"),
            Document::new("cs-2", "Python programming language"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();

        let results = store
            .similarity_search_with_score("Rust language", 2, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // Scores should be between 0 and 1.
        for (_, score) in &results {
            assert!(*score > 0.0 && *score <= 1.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires running ChromaDB instance"]
    async fn delete_documents() {
        let store = setup_store("test_delete").await;
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("cd-1", "first document"),
            Document::new("cd-2", "second document"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        store.delete(&["cd-2"]).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running ChromaDB instance"]
    async fn metadata_is_preserved() {
        let store = setup_store("test_metadata").await;
        let embeddings = FakeEmbeddings::new(64);

        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::Value::String("test".into()));
        meta.insert("page".to_string(), serde_json::json!(42));

        let docs = vec![Document::with_metadata("cm-1", "metadata test", meta)];
        store.add_documents(docs, &embeddings).await.unwrap();

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
    #[ignore = "requires running ChromaDB instance"]
    async fn empty_operations() {
        let store = setup_store("test_empty_ops").await;
        let embeddings = FakeEmbeddings::new(64);

        let ids = store.add_documents(vec![], &embeddings).await.unwrap();
        assert!(ids.is_empty());

        store.delete(&[]).await.unwrap();
    }
}
