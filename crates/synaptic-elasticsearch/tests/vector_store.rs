use synaptic_elasticsearch::{ElasticsearchConfig, ElasticsearchVectorStore};

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
        .with_vector_field("vectors");
    assert_eq!(config.vector_field, "vectors");
}

#[test]
fn config_with_content_field() {
    let config = ElasticsearchConfig::new("idx", 768)
        .with_content_field("body");
    assert_eq!(config.content_field, "body");
}

#[test]
fn config_with_auth() {
    let config = ElasticsearchConfig::new("idx", 768)
        .with_auth("elastic", "changeme");
    assert_eq!(config.username.as_deref(), Some("elastic"));
    assert_eq!(config.password.as_deref(), Some("changeme"));
}

#[test]
fn config_builder_chain() {
    let config = ElasticsearchConfig::new("documents", 1536)
        .with_url("https://es-cluster:9200")
        .with_vector_field("doc_vec")
        .with_content_field("doc_text")
        .with_auth("admin", "password");

    assert_eq!(config.index_name, "documents");
    assert_eq!(config.dims, 1536);
    assert_eq!(config.url, "https://es-cluster:9200");
    assert_eq!(config.vector_field, "doc_vec");
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
fn store_config_accessor() {
    let config = ElasticsearchConfig::new("embeddings", 1536)
        .with_url("https://es:9200")
        .with_auth("user", "pass");
    let store = ElasticsearchVectorStore::new(config);
    assert_eq!(store.config().url, "https://es:9200");
    assert_eq!(store.config().username.as_deref(), Some("user"));
}

// ---------------------------------------------------------------------------
// Integration tests — require a running Elasticsearch instance.
// Run with: cargo test -p synaptic-elasticsearch -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use synaptic_core::{Document, Embeddings, SynapticError, VectorStore};
    use synaptic_elasticsearch::{ElasticsearchConfig, ElasticsearchVectorStore};

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

    fn setup_store(index_name: &str, dims: usize) -> ElasticsearchVectorStore {
        let url = std::env::var("ELASTICSEARCH_URL")
            .unwrap_or_else(|_| "http://localhost:9200".into());
        let mut config = ElasticsearchConfig::new(index_name, dims).with_url(url);

        if let Ok(user) = std::env::var("ELASTICSEARCH_USER") {
            let pass = std::env::var("ELASTICSEARCH_PASSWORD").unwrap_or_default();
            config = config.with_auth(user, pass);
        }

        ElasticsearchVectorStore::new(config)
    }

    #[tokio::test]
    #[ignore = "requires running Elasticsearch instance"]
    async fn ensure_index_is_idempotent() {
        let store = setup_store("test_ensure_index", 64);

        // Should succeed first time (creates index).
        store.ensure_index().await.unwrap();

        // Should succeed second time (index already exists).
        store.ensure_index().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Elasticsearch instance"]
    async fn add_and_search_documents() {
        let store = setup_store("test_add_search", 64);
        store.ensure_index().await.unwrap();
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("es-1", "The quick brown fox jumps over the lazy dog"),
            Document::new("es-2", "A fast red car drives down the highway"),
            Document::new("es-3", "The lazy dog sleeps in the sun"),
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
    #[ignore = "requires running Elasticsearch instance"]
    async fn search_with_scores() {
        let store = setup_store("test_search_scores", 64);
        store.ensure_index().await.unwrap();
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("ess-1", "Rust programming language"),
            Document::new("ess-2", "Python programming language"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let results = store
            .similarity_search_with_score("Rust language", 2, &embeddings)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // First result should have the highest score.
        assert!(results[0].1 >= results[1].1);
    }

    #[tokio::test]
    #[ignore = "requires running Elasticsearch instance"]
    async fn delete_documents() {
        let store = setup_store("test_delete", 64);
        store.ensure_index().await.unwrap();
        let embeddings = FakeEmbeddings::new(64);

        let docs = vec![
            Document::new("esd-1", "first document"),
            Document::new("esd-2", "second document"),
        ];

        store.add_documents(docs, &embeddings).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        store.delete(&["esd-2"]).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Elasticsearch instance"]
    async fn metadata_is_preserved() {
        let store = setup_store("test_metadata", 64);
        store.ensure_index().await.unwrap();
        let embeddings = FakeEmbeddings::new(64);

        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::Value::String("test".into()));
        meta.insert("page".to_string(), serde_json::json!(42));

        let docs = vec![Document::with_metadata("esm-1", "metadata test", meta)];
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
    #[ignore = "requires running Elasticsearch instance"]
    async fn empty_operations() {
        let store = setup_store("test_empty_ops", 64);
        store.ensure_index().await.unwrap();
        let embeddings = FakeEmbeddings::new(64);

        let ids = store.add_documents(vec![], &embeddings).await.unwrap();
        assert!(ids.is_empty());

        store.delete(&[]).await.unwrap();
    }
}
