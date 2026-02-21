use synaptic_pinecone::{PineconeConfig, PineconeVectorStore};

#[test]
fn config_new_sets_defaults() {
    let config = PineconeConfig::new("my-api-key", "https://my-index.svc.pinecone.io");
    assert_eq!(config.api_key, "my-api-key");
    assert_eq!(config.host, "https://my-index.svc.pinecone.io");
    assert!(config.namespace.is_none());
}

#[test]
fn config_with_namespace() {
    let config = PineconeConfig::new("key", "https://host.pinecone.io")
        .with_namespace("my-ns");
    assert_eq!(config.namespace.as_deref(), Some("my-ns"));
}

#[test]
fn config_builder_chain() {
    let config = PineconeConfig::new("api-key-123", "https://idx.svc.pinecone.io")
        .with_namespace("prod");

    assert_eq!(config.api_key, "api-key-123");
    assert_eq!(config.host, "https://idx.svc.pinecone.io");
    assert_eq!(config.namespace.as_deref(), Some("prod"));
}

#[test]
fn store_new_creates_instance() {
    let config = PineconeConfig::new("key", "https://host.pinecone.io");
    let store = PineconeVectorStore::new(config);
    assert_eq!(store.config().api_key, "key");
}

#[test]
fn store_config_accessor() {
    let config = PineconeConfig::new("secret", "https://my-idx.svc.pinecone.io")
        .with_namespace("testing");
    let store = PineconeVectorStore::new(config);
    assert_eq!(store.config().host, "https://my-idx.svc.pinecone.io");
    assert_eq!(store.config().namespace.as_deref(), Some("testing"));
}

// ---------------------------------------------------------------------------
// Integration tests — require a running Pinecone index.
// Run with: PINECONE_API_KEY=... PINECONE_HOST=... cargo test -p synaptic-pinecone -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
    use synaptic_pinecone::{PineconeConfig, PineconeVectorStore};

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

    fn setup_store() -> PineconeVectorStore {
        let api_key =
            std::env::var("PINECONE_API_KEY").expect("PINECONE_API_KEY must be set");
        let host = std::env::var("PINECONE_HOST").expect("PINECONE_HOST must be set");
        let config = PineconeConfig::new(api_key, host)
            .with_namespace("synaptic-test");
        PineconeVectorStore::new(config)
    }

    #[tokio::test]
    #[ignore = "requires a running Pinecone index"]
    async fn add_and_search_documents() {
        let store = setup_store();
        let embeddings = FakeEmbeddings::new(1536);

        let docs = vec![
            Document::new("pine-1", "The quick brown fox jumps over the lazy dog"),
            Document::new("pine-2", "A fast red car drives down the highway"),
            Document::new("pine-3", "The lazy dog sleeps in the sun"),
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
    #[ignore = "requires a running Pinecone index"]
    async fn search_with_scores() {
        let store = setup_store();
        let embeddings = FakeEmbeddings::new(1536);

        let docs = vec![
            Document::new("ps-1", "Rust programming language"),
            Document::new("ps-2", "Python programming language"),
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
    #[ignore = "requires a running Pinecone index"]
    async fn delete_documents() {
        let store = setup_store();
        let embeddings = FakeEmbeddings::new(1536);

        let docs = vec![
            Document::new("pd-1", "first document to keep"),
            Document::new("pd-2", "second document to delete"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        store.delete(&["pd-2"]).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running Pinecone index"]
    async fn metadata_is_preserved() {
        let store = setup_store();
        let embeddings = FakeEmbeddings::new(1536);

        let mut meta = HashMap::new();
        meta.insert(
            "source".to_string(),
            serde_json::Value::String("test".into()),
        );
        meta.insert("page".to_string(), serde_json::json!(42));

        let docs = vec![Document::with_metadata("pm-1", "metadata test", meta)];
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
        assert_eq!(
            results[0].metadata.get("page"),
            Some(&serde_json::json!(42))
        );
    }

    #[tokio::test]
    #[ignore = "requires a running Pinecone index"]
    async fn empty_operations() {
        let store = setup_store();
        let embeddings = FakeEmbeddings::new(1536);

        let ids = store.add_documents(vec![], &embeddings).await.unwrap();
        assert!(ids.is_empty());

        store.delete(&[]).await.unwrap();
    }
}
