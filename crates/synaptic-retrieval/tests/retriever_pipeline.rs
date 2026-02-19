use std::sync::Arc;

use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::{
    BM25Retriever, ContextualCompressionRetriever, Document, DocumentCompressor, EmbeddingsFilter,
    EnsembleRetriever, Retriever,
};

#[tokio::test]
async fn bm25_single_term_query_ranks_match_first() {
    let docs = vec![
        Document::new("1", "rust language systems programming"),
        Document::new("2", "python scripting language"),
        Document::new("3", "javascript web browser"),
    ];
    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("rust", 2).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1", "document containing 'rust' should rank first");
}

#[tokio::test]
async fn bm25_empty_corpus_returns_empty() {
    let retriever = BM25Retriever::new(vec![]);
    let results = retriever.retrieve("anything", 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn ensemble_combines_bm25_retrievers() {
    let docs1 = vec![
        Document::new("1", "rust async programming"),
        Document::new("2", "python data science"),
    ];
    let docs2 = vec![
        Document::new("1", "rust async programming"),
        Document::new("3", "java enterprise"),
    ];

    let r1: Arc<dyn Retriever> = Arc::new(BM25Retriever::new(docs1));
    let r2: Arc<dyn Retriever> = Arc::new(BM25Retriever::new(docs2));

    let ensemble = EnsembleRetriever::new(vec![(r1, 0.5), (r2, 0.5)]);
    let results = ensemble.retrieve("rust", 3).await.unwrap();

    // Doc "1" appears in both retrievers, so RRF should rank it highest
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1", "shared doc should rank highest via RRF");
}

#[tokio::test]
async fn compression_with_low_threshold_passes_base_results() {
    let docs = vec![
        Document::new("1", "rust programming language is fast"),
        Document::new("2", "cooking recipes for dinner"),
        Document::new("3", "rust async runtime tokio"),
    ];

    let base: Arc<dyn Retriever> = Arc::new(BM25Retriever::new(docs));
    let embeddings = Arc::new(FakeEmbeddings::default());
    // Very low threshold so almost everything passes
    let compressor: Arc<dyn DocumentCompressor> =
        Arc::new(EmbeddingsFilter::new(embeddings, 0.01));

    let retriever = ContextualCompressionRetriever::new(base, compressor);
    let results = retriever.retrieve("rust", 5).await.unwrap();
    // BM25 finds docs with "rust" (ids 1 and 3), compression with low threshold keeps them
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"1"), "doc 1 should survive compression");
    assert!(ids.contains(&"3"), "doc 3 should survive compression");
}

#[tokio::test]
async fn bm25_respects_top_k_limit() {
    let docs: Vec<Document> = (0..20)
        .map(|i| Document::new(format!("{i}"), format!("document about topic number {i}")))
        .collect();
    let retriever = BM25Retriever::new(docs);
    let results = retriever.retrieve("document topic", 5).await.unwrap();
    assert!(results.len() <= 5, "should not return more than k=5 results");
}

#[tokio::test]
async fn ensemble_single_retriever_degenerates_to_base() {
    let docs = vec![
        Document::new("1", "hello world greeting"),
        Document::new("2", "goodbye world farewell"),
    ];
    let r: Arc<dyn Retriever> = Arc::new(BM25Retriever::new(docs));
    let ensemble = EnsembleRetriever::new(vec![(r, 1.0)]);
    let results = ensemble.retrieve("hello", 2).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "1", "only doc with 'hello' should be first");
}
