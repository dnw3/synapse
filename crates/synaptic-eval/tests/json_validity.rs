use synaptic_eval::{Evaluator, JsonValidityEvaluator};

#[tokio::test]
async fn valid_json_passes() {
    let evaluator = JsonValidityEvaluator::new();
    let result = evaluator
        .evaluate(r#"{"key": "value"}"#, "", "")
        .await
        .unwrap();
    assert!(result.passed);
    assert_eq!(result.score, 1.0);
}

#[tokio::test]
async fn invalid_json_fails() {
    let evaluator = JsonValidityEvaluator::new();
    let result = evaluator.evaluate("not json at all", "", "").await.unwrap();
    assert!(!result.passed);
    assert_eq!(result.score, 0.0);
    assert!(result.reasoning.is_some());
}

#[tokio::test]
async fn valid_json_array() {
    let evaluator = JsonValidityEvaluator::new();
    let result = evaluator.evaluate(r#"[1, 2, 3]"#, "", "").await.unwrap();
    assert!(result.passed);
    assert_eq!(result.score, 1.0);
}
