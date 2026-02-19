//! Integration tests for the `#[chain]` macro.

use serde_json::{json, Value};
use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_macros::chain;
use synaptic_runnables::Runnable;

// ---------------------------------------------------------------------------
// Basic chain
// ---------------------------------------------------------------------------

#[chain]
async fn uppercase(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default().to_uppercase();
    Ok(Value::String(s))
}

#[tokio::test]
async fn test_basic_chain_invoke() {
    let runnable = uppercase();
    let config = RunnableConfig::default();
    let result = runnable
        .invoke(json!("hello world"), &config)
        .await
        .unwrap();
    assert_eq!(result, json!("HELLO WORLD"));
}

// ---------------------------------------------------------------------------
// Chain returning structured data
// ---------------------------------------------------------------------------

#[chain]
async fn extract_name(input: Value) -> Result<Value, SynapticError> {
    let text = input
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    Ok(json!({
        "name": text.split_whitespace().next().unwrap_or(""),
        "original": text,
    }))
}

#[tokio::test]
async fn test_chain_structured() {
    let runnable = extract_name();
    let config = RunnableConfig::default();
    let result = runnable
        .invoke(json!({"text": "Alice in Wonderland"}), &config)
        .await
        .unwrap();
    assert_eq!(result.get("name").unwrap(), "Alice");
}

// ---------------------------------------------------------------------------
// Chain composition with pipe
// ---------------------------------------------------------------------------

#[chain]
async fn add_prefix(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default();
    Ok(json!(format!("PREFIX: {}", s)))
}

#[chain]
async fn add_suffix(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default();
    Ok(json!(format!("{} :SUFFIX", s)))
}

#[tokio::test]
async fn test_chain_composition() {
    let pipeline = add_prefix() | add_suffix();
    let config = RunnableConfig::default();
    let result = pipeline.invoke(json!("hello"), &config).await.unwrap();
    assert_eq!(result, json!("PREFIX: hello :SUFFIX"));
}

// ---------------------------------------------------------------------------
// Chain error propagation
// ---------------------------------------------------------------------------

#[chain]
async fn failing_chain(input: Value) -> Result<Value, SynapticError> {
    if input.is_null() {
        return Err(SynapticError::Validation("null input".into()));
    }
    Ok(input)
}

#[tokio::test]
async fn test_chain_error() {
    let runnable = failing_chain();
    let config = RunnableConfig::default();
    let result = runnable.invoke(Value::Null, &config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("null input"));
}

// ---------------------------------------------------------------------------
// Chain batch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_chain_batch() {
    let runnable = uppercase();
    let config = RunnableConfig::default();
    let results = runnable
        .batch(vec![json!("a"), json!("b"), json!("c")], &config)
        .await;
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].as_ref().unwrap(), &json!("A"));
    assert_eq!(results[1].as_ref().unwrap(), &json!("B"));
    assert_eq!(results[2].as_ref().unwrap(), &json!("C"));
}

// ---------------------------------------------------------------------------
// Typed chain: String output (no serialization)
// ---------------------------------------------------------------------------

#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[tokio::test]
async fn test_typed_chain_string_output() {
    let runnable = to_upper();
    let config = RunnableConfig::default();
    let result: String = runnable
        .invoke("hello world".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, "HELLO WORLD");
}

// ---------------------------------------------------------------------------
// Typed chain: Value output (preserves existing behavior)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_typed_chain_value_output() {
    // `uppercase()` returns BoxRunnable<Value, Value> — unchanged behavior
    let runnable = uppercase();
    let config = RunnableConfig::default();
    let result: Value = runnable
        .invoke(json!("test"), &config)
        .await
        .unwrap();
    assert_eq!(result, json!("TEST"));
}

// ---------------------------------------------------------------------------
// Typed chain: pipe composition (String -> String)
// ---------------------------------------------------------------------------

#[chain]
async fn exclaim(s: String) -> Result<String, SynapticError> {
    Ok(format!("{}!", s))
}

#[tokio::test]
async fn test_typed_chain_pipe_composition() {
    let pipeline = to_upper() | exclaim();
    let config = RunnableConfig::default();
    let result: String = pipeline
        .invoke("hello".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, "HELLO!");
}

// ---------------------------------------------------------------------------
// Typed chain: custom struct output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TextInfo {
    text: String,
    length: usize,
}

#[chain]
async fn analyze(s: String) -> Result<TextInfo, SynapticError> {
    let length = s.len();
    Ok(TextInfo { text: s, length })
}

#[tokio::test]
async fn test_typed_chain_custom_type() {
    let runnable = analyze();
    let config = RunnableConfig::default();
    let result: TextInfo = runnable
        .invoke("hello".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result, TextInfo { text: "hello".to_string(), length: 5 });
}
