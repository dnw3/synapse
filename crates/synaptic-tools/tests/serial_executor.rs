use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{SynapseError, Tool};
use synaptic_tools::{SerialToolExecutor, ToolRegistry};

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

#[tokio::test]
async fn executes_registered_tool() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).expect("register");
    let executor = SerialToolExecutor::new(registry);

    let output = executor
        .execute("echo", json!({"msg":"hi"}))
        .await
        .expect("execute");

    assert_eq!(output, json!({"echo":{"msg":"hi"}}));
}

#[tokio::test]
async fn returns_error_for_unknown_tool() {
    let registry = ToolRegistry::new();
    let executor = SerialToolExecutor::new(registry);

    let err = executor
        .execute("missing", json!({}))
        .await
        .expect_err("should fail");

    assert!(matches!(err, SynapseError::ToolNotFound(name) if name == "missing"));
}

#[tokio::test]
async fn get_returns_none_for_unregistered() {
    let registry = ToolRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn duplicate_register_overwrites() {
    let registry = ToolRegistry::new();
    registry
        .register(Arc::new(EchoTool))
        .expect("first register");
    // Registering again should succeed (overwrite)
    registry
        .register(Arc::new(EchoTool))
        .expect("second register");
    // Only one tool with name "echo" should exist
    assert!(registry.get("echo").is_some());
}

#[tokio::test]
async fn multiple_tools_in_registry() {
    struct AddTool;
    #[async_trait]
    impl Tool for AddTool {
        fn name(&self) -> &'static str {
            "add"
        }
        fn description(&self) -> &'static str {
            "Add numbers"
        }
        async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
            let a = args["a"].as_i64().unwrap_or(0);
            let b = args["b"].as_i64().unwrap_or(0);
            Ok(json!({"sum": a + b}))
        }
    }

    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    registry.register(Arc::new(AddTool)).unwrap();
    assert!(registry.get("echo").is_some());
    assert!(registry.get("add").is_some());

    let executor = SerialToolExecutor::new(registry);
    let r1 = executor
        .execute("echo", json!({"msg": "hi"}))
        .await
        .unwrap();
    assert_eq!(r1, json!({"echo": {"msg": "hi"}}));
    let r2 = executor
        .execute("add", json!({"a": 3, "b": 4}))
        .await
        .unwrap();
    assert_eq!(r2, json!({"sum": 7}));
}

#[tokio::test]
async fn sequential_executions() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    let executor = SerialToolExecutor::new(registry);

    let r1 = executor.execute("echo", json!({"n": 1})).await.unwrap();
    let r2 = executor.execute("echo", json!({"n": 2})).await.unwrap();
    let r3 = executor.execute("echo", json!({"n": 3})).await.unwrap();

    assert_eq!(r1, json!({"echo": {"n": 1}}));
    assert_eq!(r2, json!({"echo": {"n": 2}}));
    assert_eq!(r3, json!({"echo": {"n": 3}}));
}
