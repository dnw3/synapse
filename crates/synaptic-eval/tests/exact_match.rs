use synaptic_eval::{Evaluator, ExactMatchEvaluator};

#[tokio::test]
async fn exact_match_passes() {
    let evaluator = ExactMatchEvaluator::new();
    let result = evaluator.evaluate("hello", "hello", "").await.unwrap();
    assert!(result.passed);
    assert_eq!(result.score, 1.0);
}

#[tokio::test]
async fn exact_match_fails() {
    let evaluator = ExactMatchEvaluator::new();
    let result = evaluator.evaluate("hello", "world", "").await.unwrap();
    assert!(!result.passed);
    assert_eq!(result.score, 0.0);
    assert!(result.reasoning.is_some());
}

#[tokio::test]
async fn exact_match_case_insensitive() {
    let evaluator = ExactMatchEvaluator::case_insensitive();
    let result = evaluator.evaluate("Hello", "hello", "").await.unwrap();
    assert!(result.passed);
    assert_eq!(result.score, 1.0);
}
