use synaptic_eval::{Evaluator, RegexMatchEvaluator};

#[tokio::test]
async fn regex_match_passes() {
    let evaluator = RegexMatchEvaluator::new(r"^\d{3}-\d{4}$").unwrap();
    let result = evaluator.evaluate("123-4567", "", "").await.unwrap();
    assert!(result.passed);
    assert_eq!(result.score, 1.0);
}

#[tokio::test]
async fn regex_match_fails() {
    let evaluator = RegexMatchEvaluator::new(r"^\d{3}-\d{4}$").unwrap();
    let result = evaluator.evaluate("abc-defg", "", "").await.unwrap();
    assert!(!result.passed);
    assert_eq!(result.score, 0.0);
    assert!(result.reasoning.is_some());
}

#[test]
fn regex_invalid_pattern() {
    let result = RegexMatchEvaluator::new(r"[invalid");
    assert!(result.is_err());
}
