use std::sync::Arc;

use synaptic_embeddings::FakeEmbeddings;
use synaptic_eval::{EmbeddingDistanceEvaluator, Evaluator};

#[tokio::test]
async fn similar_texts_pass() {
    let embeddings = Arc::new(FakeEmbeddings::default());
    let evaluator = EmbeddingDistanceEvaluator::new(embeddings, 0.8);

    // Same text should have cosine similarity of 1.0
    let result = evaluator.evaluate("hello", "hello", "").await.unwrap();
    assert!(result.passed);
    assert!((result.score - 1.0).abs() < 1e-6);
}

#[tokio::test]
async fn threshold_filtering() {
    let embeddings = Arc::new(FakeEmbeddings::default());
    // Set a very high threshold so dissimilar texts fail
    let evaluator = EmbeddingDistanceEvaluator::new(embeddings, 0.99);

    let result = evaluator
        .evaluate("hello world", "zzzzzzzzz", "")
        .await
        .unwrap();
    // Different texts should have lower similarity and fail with high threshold
    assert!(!result.passed);
    assert!(result.score < 0.99);
}

#[tokio::test]
async fn identical_texts_score_one() {
    let embeddings = Arc::new(FakeEmbeddings::default());
    let evaluator = EmbeddingDistanceEvaluator::new(embeddings, 0.5);

    let result = evaluator
        .evaluate("same text", "same text", "")
        .await
        .unwrap();
    assert!(result.passed);
    assert!((result.score - 1.0).abs() < 1e-6);
}
