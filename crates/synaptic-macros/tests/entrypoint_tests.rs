//! Integration tests for the `#[entrypoint]` and `#[task]` macros.

use serde_json::{json, Value};
use synaptic_core::SynapticError;
use synaptic_macros::{entrypoint, task};

// ===========================================================================
// #[entrypoint] tests
// ===========================================================================

// ---------------------------------------------------------------------------
// Basic entrypoint
// ---------------------------------------------------------------------------

#[entrypoint]
async fn echo_workflow(input: Value) -> Result<Value, SynapticError> {
    Ok(input)
}

#[tokio::test]
async fn test_entrypoint_invoke() {
    let ep = echo_workflow();
    let result = ep.invoke(json!({"msg": "hello"})).await.unwrap();
    assert_eq!(result, json!({"msg": "hello"}));
}

#[tokio::test]
async fn test_entrypoint_config_name() {
    let ep = echo_workflow();
    assert_eq!(ep.config.name, "echo_workflow");
}

// ---------------------------------------------------------------------------
// Entrypoint with checkpointer
// ---------------------------------------------------------------------------

#[entrypoint(checkpointer = "memory")]
async fn persisted_workflow(input: Value) -> Result<Value, SynapticError> {
    let mut out = input.clone();
    out["persisted"] = json!(true);
    Ok(out)
}

#[tokio::test]
async fn test_entrypoint_config_checkpointer() {
    let ep = persisted_workflow();
    assert_eq!(ep.config.checkpointer, Some("memory"));
}

// ---------------------------------------------------------------------------
// Entrypoint error propagation
// ---------------------------------------------------------------------------

#[entrypoint]
async fn failing_workflow(input: Value) -> Result<Value, SynapticError> {
    if input.is_null() {
        return Err(SynapticError::Validation("null input not allowed".into()));
    }
    Ok(input)
}

#[tokio::test]
async fn test_entrypoint_error_propagation() {
    let ep = failing_workflow();
    let result = ep.invoke(Value::Null).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("null input not allowed"));
}

// ---------------------------------------------------------------------------
// Entrypoint with custom name
// ---------------------------------------------------------------------------

#[entrypoint(name = "my_custom_ep")]
async fn aliased_workflow(input: Value) -> Result<Value, SynapticError> {
    Ok(json!({"received": input}))
}

#[tokio::test]
async fn test_entrypoint_custom_name() {
    let ep = aliased_workflow();
    assert_eq!(ep.config.name, "my_custom_ep");
    // Also verify it still works
    let result = ep.invoke(json!(42)).await.unwrap();
    assert_eq!(result, json!({"received": 42}));
}

// ---------------------------------------------------------------------------
// Entrypoint with both name and checkpointer
// ---------------------------------------------------------------------------

#[entrypoint(name = "combo", checkpointer = "sqlite")]
async fn combo_workflow(input: Value) -> Result<Value, SynapticError> {
    Ok(input)
}

#[tokio::test]
async fn test_entrypoint_name_and_checkpointer() {
    let ep = combo_workflow();
    assert_eq!(ep.config.name, "combo");
    assert_eq!(ep.config.checkpointer, Some("sqlite"));
}

// ===========================================================================
// #[task] tests
// ===========================================================================

// ---------------------------------------------------------------------------
// Basic task
// ---------------------------------------------------------------------------

#[task]
async fn greet(name: String) -> Result<String, SynapticError> {
    Ok(format!("Hello, {}!", name))
}

#[tokio::test]
async fn test_task_basic() {
    let result = greet("Alice".to_string()).await.unwrap();
    assert_eq!(result, "Hello, Alice!");
}

// ---------------------------------------------------------------------------
// Task preserves return type
// ---------------------------------------------------------------------------

#[task]
async fn parse_number(s: String) -> Result<i64, SynapticError> {
    s.parse::<i64>()
        .map_err(|e| SynapticError::Parsing(e.to_string()))
}

#[tokio::test]
async fn test_task_preserves_return_type() {
    let result: Result<i64, SynapticError> = parse_number("42".to_string()).await;
    assert_eq!(result.unwrap(), 42i64);
}

// ---------------------------------------------------------------------------
// Task with multiple params
// ---------------------------------------------------------------------------

#[task]
async fn add(a: i64, b: i64) -> Result<i64, SynapticError> {
    Ok(a + b)
}

#[tokio::test]
async fn test_task_with_multiple_params() {
    let result = add(3, 4).await.unwrap();
    assert_eq!(result, 7);
}

// ---------------------------------------------------------------------------
// Task error propagation
// ---------------------------------------------------------------------------

#[task]
async fn must_be_positive(n: i64) -> Result<i64, SynapticError> {
    if n <= 0 {
        return Err(SynapticError::Validation("must be positive".into()));
    }
    Ok(n)
}

#[tokio::test]
async fn test_task_error_propagation() {
    let result = must_be_positive(-1).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be positive"));
}

// ---------------------------------------------------------------------------
// Task with custom name
// ---------------------------------------------------------------------------

#[task(name = "weather_fetcher")]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("Sunny in {}", city))
}

#[tokio::test]
async fn test_task_with_name() {
    // The custom name is embedded as a const in the generated code.
    // We verify the function still works correctly.
    let result = fetch_weather("Paris".to_string()).await.unwrap();
    assert_eq!(result, "Sunny in Paris");
}
