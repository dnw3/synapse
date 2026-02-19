use serde_json::json;
use synaptic_core::{SynapticError, Tool};
use synaptic_macros::tool;
use synaptic_tools::HandleErrorTool;

/// Always fails
#[tool(name = "failing")]
async fn fail_tool() -> Result<String, SynapticError> {
    Err(SynapticError::Tool("something went wrong".to_string()))
}

/// Always succeeds
#[tool(name = "succeeding")]
async fn succeed_tool() -> Result<serde_json::Value, SynapticError> {
    Ok(json!({"ok": true}))
}

#[tokio::test]
async fn default_handler_returns_error_string() {
    let inner = fail_tool();
    let wrapper = HandleErrorTool::new(inner);

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!("tool error: something went wrong"));
}

#[tokio::test]
async fn custom_handler_transforms_error() {
    let inner = fail_tool();
    let wrapper = HandleErrorTool::with_handler(inner, |err| format!("CUSTOM: {}", err));

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!("CUSTOM: tool error: something went wrong"));
}

#[tokio::test]
async fn success_passes_through() {
    let inner = succeed_tool();
    let wrapper = HandleErrorTool::new(inner);

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!({"ok": true}));
}

#[tokio::test]
async fn delegates_name_and_description() {
    let inner = fail_tool();
    let wrapper = HandleErrorTool::new(inner);

    assert_eq!(wrapper.name(), "failing");
    assert_eq!(wrapper.description(), "Always fails");
}
