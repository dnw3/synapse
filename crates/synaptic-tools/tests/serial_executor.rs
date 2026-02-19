use serde_json::{json, Value};
use synaptic_core::SynapticError;
use synaptic_macros::tool;
use synaptic_tools::{SerialToolExecutor, ToolRegistry};

/// Echo input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(json!({"echo": args}))
}

/// Add two numbers.
#[tool(name = "add")]
async fn add_tool(a: i64, b: i64) -> Result<serde_json::Value, SynapticError> {
    Ok(json!({"sum": a + b}))
}

#[tokio::test]
async fn executes_registered_tool() {
    let registry = ToolRegistry::new();
    registry.register(echo()).expect("register");
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

    assert!(matches!(err, SynapticError::ToolNotFound(name) if name == "missing"));
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
        .register(echo())
        .expect("first register");
    // Registering again should succeed (overwrite)
    registry
        .register(echo())
        .expect("second register");
    // Only one tool with name "echo" should exist
    assert!(registry.get("echo").is_some());
}

#[tokio::test]
async fn multiple_tools_in_registry() {
    let registry = ToolRegistry::new();
    registry.register(echo()).unwrap();
    registry.register(add_tool()).unwrap();
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
    registry.register(echo()).unwrap();
    let executor = SerialToolExecutor::new(registry);

    let r1 = executor.execute("echo", json!({"n": 1})).await.unwrap();
    let r2 = executor.execute("echo", json!({"n": 2})).await.unwrap();
    let r3 = executor.execute("echo", json!({"n": 3})).await.unwrap();

    assert_eq!(r1, json!({"echo": {"n": 1}}));
    assert_eq!(r2, json!({"echo": {"n": 2}}));
    assert_eq!(r3, json!({"echo": {"n": 3}}));
}
