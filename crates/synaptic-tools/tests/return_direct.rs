use serde_json::json;
use synaptic_core::{SynapticError, Tool};
use synaptic_macros::tool;
use synaptic_tools::ReturnDirectTool;

/// A simple calculator
#[tool(name = "calculator")]
async fn calc(a: f64, b: f64) -> Result<serde_json::Value, SynapticError> {
    Ok(json!({"result": a + b}))
}

#[tokio::test]
async fn return_direct_delegates_to_inner() {
    let inner = calc();
    let wrapper = ReturnDirectTool::new(inner);

    assert_eq!(wrapper.name(), "calculator");
    assert_eq!(wrapper.description(), "A simple calculator");
    assert!(wrapper.is_return_direct());

    let result = wrapper.call(json!({"a": 2, "b": 3})).await.unwrap();
    assert_eq!(result, json!({"result": 5.0}));
}

#[tokio::test]
async fn return_direct_tool_implements_tool_trait() {
    use std::sync::Arc;
    use synaptic_core::Tool;

    let inner = calc();
    let wrapper: Arc<dyn Tool> = Arc::new(ReturnDirectTool::new(inner));

    let result = wrapper.call(json!({"a": 10, "b": 20})).await.unwrap();
    assert_eq!(result, json!({"result": 30.0}));
}
