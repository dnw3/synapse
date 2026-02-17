use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{SynapseError, Tool};
use synaptic_tools::ReturnDirectTool;

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }

    fn description(&self) -> &'static str {
        "A simple calculator"
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"result": a + b}))
    }
}

#[tokio::test]
async fn return_direct_delegates_to_inner() {
    let inner = Arc::new(CalculatorTool);
    let wrapper = ReturnDirectTool::new(inner);

    assert_eq!(wrapper.name(), "calculator");
    assert_eq!(wrapper.description(), "A simple calculator");
    assert!(wrapper.is_return_direct());

    let result = wrapper.call(json!({"a": 2, "b": 3})).await.unwrap();
    assert_eq!(result, json!({"result": 5.0}));
}

#[tokio::test]
async fn return_direct_tool_implements_tool_trait() {
    let inner = Arc::new(CalculatorTool);
    let wrapper: Arc<dyn Tool> = Arc::new(ReturnDirectTool::new(inner));

    let result = wrapper.call(json!({"a": 10, "b": 20})).await.unwrap();
    assert_eq!(result, json!({"result": 30.0}));
}
