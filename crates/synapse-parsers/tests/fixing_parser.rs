use std::sync::Arc;

use synapse_core::{ChatResponse, Message, RunnableConfig};
use synapse_models::ScriptedChatModel;
use synapse_parsers::{JsonOutputParser, OutputFixingParser};
use synapse_runnables::Runnable;

#[tokio::test]
async fn succeeds_on_first_attempt() {
    // The inner parser succeeds immediately, so the LLM is never called.
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let parser = OutputFixingParser::new(Box::new(JsonOutputParser), model);
    let config = RunnableConfig::default();

    let result = parser
        .invoke(r#"{"key": "value"}"#.to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"key": "value"}));
}

#[tokio::test]
async fn fixes_invalid_output_via_llm() {
    // The inner parser fails on invalid JSON, then the LLM returns valid JSON.
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(r#"{"key": "value"}"#),
        usage: None,
    }]));
    let parser = OutputFixingParser::new(Box::new(JsonOutputParser), model);
    let config = RunnableConfig::default();

    let result = parser
        .invoke("not valid json".to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"key": "value"}));
}

#[tokio::test]
async fn returns_error_after_max_retries_exhausted() {
    // The LLM also returns invalid JSON, so after retries it should fail.
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("still not valid json"),
        usage: None,
    }]));
    let parser = OutputFixingParser::new(Box::new(JsonOutputParser), model);
    let config = RunnableConfig::default();

    let err = parser
        .invoke("not valid json".to_string(), &config)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid JSON"));
}

#[tokio::test]
async fn multiple_retries_eventually_succeeds() {
    // First LLM attempt returns bad JSON, second returns valid JSON.
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("still bad"),
            usage: None,
        },
        ChatResponse {
            message: Message::ai(r#"{"fixed": true}"#),
            usage: None,
        },
    ]));
    let parser = OutputFixingParser::new(Box::new(JsonOutputParser), model).with_max_retries(2);
    let config = RunnableConfig::default();

    let result = parser
        .invoke("broken json".to_string(), &config)
        .await
        .unwrap();

    assert_eq!(result, serde_json::json!({"fixed": true}));
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
        ChatResponse {
            message: Message::ai("bad3"),
            usage: None,
        },
    ]));
    let parser = OutputFixingParser::new(Box::new(JsonOutputParser), model).with_max_retries(3);
    let config = RunnableConfig::default();

    let err = parser
        .invoke("invalid".to_string(), &config)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid JSON"));
}
