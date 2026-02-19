use serde_json::json;
use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_macros::chain;
use synaptic_runnables::{Runnable, RunnableAssign, RunnablePassthrough, RunnablePick};

// ---------------------------------------------------------------------------
// #[chain] functions replacing inline RunnableLambda closures
// ---------------------------------------------------------------------------

#[chain]
async fn uppercase_name(input: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
    let name = input["name"].as_str().unwrap_or("").to_uppercase();
    Ok(serde_json::Value::String(name))
}

#[chain]
async fn always_true(_input: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
    Ok(serde_json::Value::Bool(true))
}

#[chain]
async fn double_n(input: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
    let n = input["n"].as_i64().unwrap_or(0) * 2;
    Ok(json!(n))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn assign_merges_computed_keys() {
    let branches = vec![("upper".to_string(), uppercase_name())];
    let assign = RunnableAssign::new(branches);
    let config = RunnableConfig::default();
    let input = json!({"name": "alice", "age": 30});
    let result = assign.invoke(input, &config).await.unwrap();

    assert_eq!(result["name"], "alice");
    assert_eq!(result["age"], 30);
    assert_eq!(result["upper"], "ALICE");
}

#[tokio::test]
async fn assign_preserves_input_keys() {
    let branches = vec![("computed".to_string(), always_true())];
    let assign = RunnableAssign::new(branches);
    let config = RunnableConfig::default();
    let input = json!({"x": 1, "y": 2});
    let result = assign.invoke(input, &config).await.unwrap();

    assert_eq!(result["x"], 1);
    assert_eq!(result["y"], 2);
    assert_eq!(result["computed"], true);
}

#[tokio::test]
async fn passthrough_assign_factory() {
    let assign =
        RunnablePassthrough::assign(vec![("doubled".to_string(), double_n())]);
    let config = RunnableConfig::default();
    let result = assign.invoke(json!({"n": 5}), &config).await.unwrap();
    assert_eq!(result["n"], 5);
    assert_eq!(result["doubled"], 10);
}

#[tokio::test]
async fn pick_single_key() {
    let pick = RunnablePick::new(vec!["name".to_string()]);
    let config = RunnableConfig::default();
    let result = pick
        .invoke(json!({"name": "alice", "age": 30}), &config)
        .await
        .unwrap();
    assert_eq!(result, json!({"name": "alice"}));
}

#[tokio::test]
async fn pick_multiple_keys() {
    let pick = RunnablePick::new(vec!["a".to_string(), "c".to_string()]);
    let config = RunnableConfig::default();
    let result = pick
        .invoke(json!({"a": 1, "b": 2, "c": 3}), &config)
        .await
        .unwrap();
    assert_eq!(result, json!({"a": 1, "c": 3}));
}

#[tokio::test]
async fn pick_missing_keys_ignored() {
    let pick = RunnablePick::new(vec!["a".to_string(), "missing".to_string()]);
    let config = RunnableConfig::default();
    let result = pick.invoke(json!({"a": 1, "b": 2}), &config).await.unwrap();
    assert_eq!(result, json!({"a": 1}));
}
