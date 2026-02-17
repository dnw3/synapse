use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synapse_core::{SynapseError, Tool};
use synapse_tools::HandleErrorTool;

struct FailingTool;

#[async_trait]
impl Tool for FailingTool {
    fn name(&self) -> &'static str {
        "failing"
    }

    fn description(&self) -> &'static str {
        "Always fails"
    }

    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        Err(SynapseError::Tool("something went wrong".to_string()))
    }
}

struct SucceedingTool;

#[async_trait]
impl Tool for SucceedingTool {
    fn name(&self) -> &'static str {
        "succeeding"
    }

    fn description(&self) -> &'static str {
        "Always succeeds"
    }

    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        Ok(json!({"ok": true}))
    }
}

#[tokio::test]
async fn default_handler_returns_error_string() {
    let inner = Arc::new(FailingTool);
    let wrapper = HandleErrorTool::new(inner);

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!("tool error: something went wrong"));
}

#[tokio::test]
async fn custom_handler_transforms_error() {
    let inner = Arc::new(FailingTool);
    let wrapper = HandleErrorTool::with_handler(inner, |err| format!("CUSTOM: {}", err));

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!("CUSTOM: tool error: something went wrong"));
}

#[tokio::test]
async fn success_passes_through() {
    let inner = Arc::new(SucceedingTool);
    let wrapper = HandleErrorTool::new(inner);

    let result = wrapper.call(json!({})).await.unwrap();
    assert_eq!(result, json!({"ok": true}));
}

#[tokio::test]
async fn delegates_name_and_description() {
    let inner = Arc::new(FailingTool);
    let wrapper = HandleErrorTool::new(inner);

    assert_eq!(wrapper.name(), "failing");
    assert_eq!(wrapper.description(), "Always fails");
}
