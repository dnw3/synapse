use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{SynapseError, Tool};
use synaptic_tools::{ParallelToolExecutor, ToolRegistry};

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &'static str {
        "Echo input"
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        Ok(json!({"echo": args}))
    }
}

struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &'static str {
        "add"
    }

    fn description(&self) -> &'static str {
        "Add two numbers"
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"sum": a + b}))
    }
}

#[tokio::test]
async fn executes_multiple_tools_concurrently() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    registry.register(Arc::new(AddTool)).unwrap();

    let executor = ParallelToolExecutor::new(registry);

    let calls = vec![
        ("echo".to_string(), json!({"msg": "hello"})),
        ("add".to_string(), json!({"a": 1, "b": 2})),
        ("echo".to_string(), json!({"msg": "world"})),
    ];

    let results = executor.execute_all(calls).await;

    assert_eq!(results.len(), 3);
    assert_eq!(
        results[0].as_ref().unwrap(),
        &json!({"echo": {"msg": "hello"}})
    );
    assert_eq!(results[1].as_ref().unwrap(), &json!({"sum": 3.0}));
    assert_eq!(
        results[2].as_ref().unwrap(),
        &json!({"echo": {"msg": "world"}})
    );
}

#[tokio::test]
async fn returns_error_for_unknown_tool() {
    let registry = ToolRegistry::new();
    let executor = ParallelToolExecutor::new(registry);

    let calls = vec![("missing".to_string(), json!({}))];
    let results = executor.execute_all(calls).await;

    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
    assert!(matches!(
        results[0].as_ref().unwrap_err(),
        SynapseError::ToolNotFound(name) if name == "missing"
    ));
}

#[tokio::test]
async fn empty_calls_returns_empty() {
    let registry = ToolRegistry::new();
    let executor = ParallelToolExecutor::new(registry);

    let results = executor.execute_all(vec![]).await;
    assert!(results.is_empty());
}

#[tokio::test]
async fn mixed_success_and_failure() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();

    let executor = ParallelToolExecutor::new(registry);

    let calls = vec![
        ("echo".to_string(), json!({"ok": true})),
        ("nonexistent".to_string(), json!({})),
        ("echo".to_string(), json!({"ok": false})),
    ];

    let results = executor.execute_all(calls).await;

    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(results[1].is_err());
    assert!(results[2].is_ok());
}
