use std::sync::Arc;

use synaptic_core::{ChatResponse, Message};
use synaptic_eval::{Evaluator, LLMJudgeEvaluator};
use synaptic_models::ScriptedChatModel;

#[tokio::test]
async fn llm_judge_high_score() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("8"),
        usage: None,
    }]));

    let evaluator = LLMJudgeEvaluator::new(model);
    let result = evaluator
        .evaluate("Paris", "Paris", "What is the capital of France?")
        .await
        .unwrap();

    assert!(result.passed);
    assert!((result.score - 0.8).abs() < 1e-6);
}

#[tokio::test]
async fn llm_judge_low_score() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("2"),
        usage: None,
    }]));

    let evaluator = LLMJudgeEvaluator::new(model);
    let result = evaluator
        .evaluate("Berlin", "Paris", "What is the capital of France?")
        .await
        .unwrap();

    assert!(!result.passed);
    assert!((result.score - 0.2).abs() < 1e-6);
}

#[tokio::test]
async fn llm_judge_unparseable_score() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("not a number"),
        usage: None,
    }]));

    let evaluator = LLMJudgeEvaluator::new(model);
    let result = evaluator.evaluate("prediction", "reference", "input").await;

    // Should error or return a default score when response isn't a number
    assert!(result.is_err() || result.unwrap().score == 0.0);
}

#[tokio::test]
async fn llm_judge_boundary_score() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("10"),
        usage: None,
    }]));

    let evaluator = LLMJudgeEvaluator::new(model);
    let result = evaluator
        .evaluate("perfect", "perfect", "test")
        .await
        .unwrap();

    assert!(result.passed);
    assert!((result.score - 1.0).abs() < 1e-6);
}
