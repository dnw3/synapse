use serde_json::json;
use synaptic_core::RunnableConfig;
use synaptic_parsers::JsonOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn parses_valid_json_object() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let result = parser
        .invoke(r#"{"name": "Alice", "age": 30}"#.to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, json!({"name": "Alice", "age": 30}));
}

#[tokio::test]
async fn parses_json_array() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let result = parser
        .invoke("[1, 2, 3]".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[tokio::test]
async fn returns_error_on_invalid_json() {
    let parser = JsonOutputParser;
    let config = RunnableConfig::default();
    let err = parser
        .invoke("not json".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid JSON"));
}
