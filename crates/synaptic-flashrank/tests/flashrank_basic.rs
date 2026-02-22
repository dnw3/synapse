use synaptic_core::Document;
use synaptic_flashrank::{FlashRankConfig, FlashRankReranker};

#[tokio::test]
async fn rerank_basic() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let docs = vec![
        Document::new(
            "d1",
            "Paris is the capital of France and home to the Eiffel Tower.",
        ),
        Document::new("d2", "Berlin is the capital of Germany."),
        Document::new("d3", "The weather is sunny today."),
    ];
    let results = reranker.rerank("capital of France", docs, 2).await.unwrap();
    assert_eq!(results.len(), 2);
    // Paris doc should score highest (contains "capital" and "france")
    assert!(results[0].0.content.contains("Paris"));
    assert!(results[0].1 >= results[1].1);
}

#[tokio::test]
async fn rerank_empty_docs() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let results = reranker.rerank("query", vec![], 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn rerank_top_k_limit() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let docs = (0..5)
        .map(|i| Document::new(&format!("doc{i}"), &format!("document number {i}")))
        .collect();
    let results = reranker.rerank("document", docs, 3).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn rerank_top_k_larger_than_docs() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let docs = vec![
        Document::new("d1", "hello world"),
        Document::new("d2", "foo bar"),
    ];
    let results = reranker.rerank("hello", docs, 10).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn rerank_scores_sorted() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let docs = vec![
        Document::new("d1", "Rust programming language systems"),
        Document::new(
            "d2",
            "Rust is great for systems programming and memory safety rust rust",
        ),
        Document::new("d3", "Python is a popular scripting language"),
    ];
    let results = reranker
        .rerank("Rust systems programming", docs, 3)
        .await
        .unwrap();
    // All results returned, sorted by score descending
    for window in results.windows(2) {
        assert!(window[0].1 >= window[1].1);
    }
}

#[tokio::test]
async fn rerank_empty_query() {
    let reranker = FlashRankReranker::new(FlashRankConfig::default());
    let docs = vec![Document::new("d1", "hello"), Document::new("d2", "world")];
    let results = reranker.rerank("", docs, 2).await.unwrap();
    // Empty query returns results (with score 0)
    assert_eq!(results.len(), 2);
}
