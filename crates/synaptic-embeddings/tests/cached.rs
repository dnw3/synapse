use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapseError;
use synaptic_embeddings::{CacheBackedEmbeddings, Embeddings, FakeEmbeddings};

/// A counting wrapper around FakeEmbeddings to verify cache behavior.
struct CountingEmbeddings {
    inner: FakeEmbeddings,
    embed_query_calls: AtomicUsize,
    embed_documents_calls: AtomicUsize,
}

impl CountingEmbeddings {
    fn new() -> Self {
        Self {
            inner: FakeEmbeddings::new(4),
            embed_query_calls: AtomicUsize::new(0),
            embed_documents_calls: AtomicUsize::new(0),
        }
    }

    fn query_call_count(&self) -> usize {
        self.embed_query_calls.load(Ordering::SeqCst)
    }

    fn document_call_count(&self) -> usize {
        self.embed_documents_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Embeddings for CountingEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapseError> {
        self.embed_documents_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.embed_documents(texts).await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapseError> {
        self.embed_query_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.embed_query(text).await
    }
}

#[tokio::test]
async fn embed_query_caches_results() {
    let counting = Arc::new(CountingEmbeddings::new());
    let cached = CacheBackedEmbeddings::new(counting.clone());

    let result1 = cached.embed_query("hello world").await.unwrap();
    assert_eq!(counting.query_call_count(), 1);

    let result2 = cached.embed_query("hello world").await.unwrap();
    assert_eq!(counting.query_call_count(), 1); // No additional call

    assert_eq!(result1, result2);
}

#[tokio::test]
async fn embed_query_different_texts_calls_inner() {
    let counting = Arc::new(CountingEmbeddings::new());
    let cached = CacheBackedEmbeddings::new(counting.clone());

    cached.embed_query("hello").await.unwrap();
    assert_eq!(counting.query_call_count(), 1);

    cached.embed_query("world").await.unwrap();
    assert_eq!(counting.query_call_count(), 2);
}

#[tokio::test]
async fn embed_documents_caches_results() {
    let counting = Arc::new(CountingEmbeddings::new());
    let cached = CacheBackedEmbeddings::new(counting.clone());

    let result1 = cached.embed_documents(&["hello", "world"]).await.unwrap();
    assert_eq!(counting.document_call_count(), 1);
    assert_eq!(result1.len(), 2);

    // Same texts again - should use cache, no inner call
    let result2 = cached.embed_documents(&["hello", "world"]).await.unwrap();
    assert_eq!(counting.document_call_count(), 1); // No additional call

    assert_eq!(result1, result2);
}

#[tokio::test]
async fn embed_documents_partial_cache_hit() {
    let counting = Arc::new(CountingEmbeddings::new());
    let cached = CacheBackedEmbeddings::new(counting.clone());

    // First call caches "hello" and "world"
    cached.embed_documents(&["hello", "world"]).await.unwrap();
    assert_eq!(counting.document_call_count(), 1);

    // Second call: "hello" is cached, "new" is not
    let result = cached.embed_documents(&["hello", "new"]).await.unwrap();
    assert_eq!(counting.document_call_count(), 2);
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn embed_query_and_documents_share_cache() {
    let counting = Arc::new(CountingEmbeddings::new());
    let cached = CacheBackedEmbeddings::new(counting.clone());

    // Cache via embed_query
    let query_result = cached.embed_query("hello").await.unwrap();
    assert_eq!(counting.query_call_count(), 1);

    // embed_documents should find "hello" cached
    let doc_results = cached.embed_documents(&["hello"]).await.unwrap();
    assert_eq!(counting.document_call_count(), 0); // No call to embed_documents

    assert_eq!(query_result, doc_results[0]);
}
