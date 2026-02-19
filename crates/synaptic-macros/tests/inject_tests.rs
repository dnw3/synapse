//! Integration tests for `#[inject]` support in the `#[tool]` macro.

use serde_json::json;
use std::sync::Arc;
use synaptic_core::{RuntimeAwareTool, SynapticError, ToolRuntime};
use synaptic_macros::tool;

// ---------------------------------------------------------------------------
// Tool with #[inject(tool_call_id)]
// ---------------------------------------------------------------------------

/// A tool that echoes its own tool call ID.
#[tool]
async fn echo_id(
    /// The message to echo
    message: String,
    #[inject(tool_call_id)] call_id: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}: {}", call_id, message))
}

#[tokio::test]
async fn test_inject_tool_call_id() {
    let t: Arc<dyn RuntimeAwareTool> = echo_id();

    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: None,
        tool_call_id: "call_abc123".to_string(),
        config: None,
    };

    let result = t
        .call_with_runtime(json!({"message": "hello"}), runtime)
        .await
        .unwrap();

    assert_eq!(result, json!("call_abc123: hello"));
}

#[tokio::test]
async fn test_inject_tool_call_id_not_in_schema() {
    let t: Arc<dyn RuntimeAwareTool> = echo_id();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();

    // "message" should be in the schema
    assert!(props.get("message").is_some());

    // "call_id" (injected) should NOT be in the schema
    assert!(props.get("call_id").is_none());
}

// ---------------------------------------------------------------------------
// Tool with #[inject(state)]
// ---------------------------------------------------------------------------

/// A tool that reads state.
#[tool]
async fn read_state(
    /// The key to look up
    key: String,
    #[inject(state)] state: serde_json::Value,
) -> Result<String, SynapticError> {
    let val = state
        .get(&key)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "not found".into());
    Ok(val)
}

#[tokio::test]
async fn test_inject_state() {
    let t: Arc<dyn RuntimeAwareTool> = read_state();

    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: Some(json!({"name": "Alice", "age": 30})),
        tool_call_id: String::new(),
        config: None,
    };

    let result = t
        .call_with_runtime(json!({"key": "name"}), runtime)
        .await
        .unwrap();

    assert_eq!(result, json!("\"Alice\""));
}

#[tokio::test]
async fn test_inject_state_not_in_schema() {
    let t: Arc<dyn RuntimeAwareTool> = read_state();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();

    assert!(props.get("key").is_some());
    assert!(props.get("state").is_none());
}

// ---------------------------------------------------------------------------
// Tool with mixed injected and regular params
// ---------------------------------------------------------------------------

/// A tool that combines injected and regular params.
#[tool]
async fn mixed_tool(
    /// The user input
    input: String,
    /// An optional tag
    tag: Option<String>,
    #[inject(tool_call_id)] call_id: String,
    #[inject(state)] state: serde_json::Value,
) -> Result<String, SynapticError> {
    let tag_str = tag.unwrap_or_else(|| "none".into());
    let user = state
        .get("user")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    Ok(format!(
        "id={}, input={}, tag={}, user={}",
        call_id, input, tag_str, user
    ))
}

#[tokio::test]
async fn test_inject_mixed() {
    let t: Arc<dyn RuntimeAwareTool> = mixed_tool();

    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: Some(json!({"user": "Bob"})),
        tool_call_id: "call_xyz".to_string(),
        config: None,
    };

    let result = t
        .call_with_runtime(json!({"input": "hello", "tag": "greet"}), runtime)
        .await
        .unwrap();

    assert_eq!(
        result,
        json!("id=call_xyz, input=hello, tag=greet, user=Bob")
    );
}

#[tokio::test]
async fn test_inject_mixed_schema() {
    let t: Arc<dyn RuntimeAwareTool> = mixed_tool();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();

    // Regular params should be in schema
    assert!(props.get("input").is_some());
    assert!(props.get("tag").is_some());

    // Injected params should NOT be in schema
    assert!(props.get("call_id").is_none());
    assert!(props.get("state").is_none());
}

#[tokio::test]
async fn test_inject_mixed_required() {
    let t: Arc<dyn RuntimeAwareTool> = mixed_tool();
    let params = t.parameters().unwrap();
    let required = params.get("required").unwrap().as_array().unwrap();

    // "input" is required (not Option, no default)
    assert!(required.contains(&json!("input")));

    // "tag" is Option so should NOT be required
    assert!(!required.contains(&json!("tag")));

    // Injected params should not appear in required
    assert!(!required.contains(&json!("call_id")));
    assert!(!required.contains(&json!("state")));
}

// ---------------------------------------------------------------------------
// RuntimeAwareTool trait methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_runtime_aware_tool_name() {
    let t: Arc<dyn RuntimeAwareTool> = echo_id();
    assert_eq!(t.name(), "echo_id");
}

#[tokio::test]
async fn test_runtime_aware_tool_description() {
    let t: Arc<dyn RuntimeAwareTool> = echo_id();
    assert_eq!(t.description(), "A tool that echoes its own tool call ID.");
}

#[tokio::test]
async fn test_runtime_aware_as_tool_definition() {
    let t: Arc<dyn RuntimeAwareTool> = echo_id();
    let def = t.as_tool_definition();
    assert_eq!(def.name, "echo_id");
    assert_eq!(def.description, "A tool that echoes its own tool call ID.");
    assert!(def.parameters.get("properties").is_some());
}
