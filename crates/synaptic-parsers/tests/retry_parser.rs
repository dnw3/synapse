use std::sync::Arc;

use synaptic_core::{ChatResponse, Message, RunnableConfig};
use synaptic_models::ScriptedChatModel;
use synaptic_parsers::{JsonOutputParser, RetryOutputParser};
use synaptic_runnables::Runnable;

#[tokio::test]
async fn succeeds_on_first_attempt() {
    // The inner parser succeeds immediately, so the LLM is never called.
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let parser = RetryOutputParser::new(
        Box::new(JsonOutputParser),
        model,
        "Generate a JSON object with a name field.",
    );
    let config = RunnableConfig::default();

    let result = parser
        .invoke(r#"{"name": "Alice"}"#.to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"name": "Alice"}));
}

#[tokio::test]
async fn fixes_invalid_output_with_prompt_context() {
    // The inner parser fails, then the LLM returns valid JSON using prompt context.
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(r#"{"name": "Alice"}"#),
        usage: None,
    }]));
    let parser = RetryOutputParser::new(
        Box::new(JsonOutputParser),
        model,
        "Generate a JSON object with a name field.",
    );
    let config = RunnableConfig::default();

    let result = parser
        .invoke("name: Alice".to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"name": "Alice"}));
}

#[tokio::test]
async fn returns_error_after_max_retries_exhausted() {
    // The LLM also returns invalid JSON, so after retries it should fail.
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("still not json"),
        usage: None,
    }]));
    let parser = RetryOutputParser::new(Box::new(JsonOutputParser), model, "Generate valid JSON.");
    let config = RunnableConfig::default();

    let err = parser
        .invoke("bad input".to_string(), &config)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid JSON"));
}

#[tokio::test]
async fn multiple_retries_eventually_succeeds() {
    // First LLM attempt returns bad JSON, second returns valid JSON.
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("still broken"),
            usage: None,
        },
        ChatResponse {
            message: Message::ai(r#"{"status": "ok"}"#),
            usage: None,
        },
    ]));
    let parser = RetryOutputParser::new(
        Box::new(JsonOutputParser),
        model,
        "Generate a status JSON object.",
    )
    .with_max_retries(2);
    let config = RunnableConfig::default();

    let result = parser
        .invoke("not json".to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"status": "ok"}));
}

#[tokio::test]
async fn multiple_retries_all_fail() {
    // All LLM attempts return bad JSON.
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("bad1"),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("bad2"),
            usage: None,
        },
    ]));
    let parser = RetryOutputParser::new(Box::new(JsonOutputParser), model, "Generate valid JSON.")
        .with_max_retries(2);
    let config = RunnableConfig::default();

    let err = parser
        .invoke("invalid".to_string(), &config)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid JSON"));
}
