use serde_json::json;
use std::sync::Arc;
use synaptic_core::{
    RunnableConfig, Runtime, RuntimeAwareTool, RuntimeAwareToolAdapter, SynapticError, Tool,
    ToolRuntime,
};

// A simple RuntimeAwareTool for testing
struct EchoRuntimeTool;

#[async_trait::async_trait]
impl RuntimeAwareTool for EchoRuntimeTool {
    fn name(&self) -> &'static str {
        "echo_runtime"
    }

    fn description(&self) -> &'static str {
        "Echoes runtime info"
    }

    async fn call_with_runtime(
        &self,
        args: serde_json::Value,
        runtime: ToolRuntime,
    ) -> Result<serde_json::Value, SynapticError> {
        Ok(json!({
            "args": args,
            "tool_call_id": runtime.tool_call_id,
            "has_store": runtime.store.is_some(),
            "has_state": runtime.state.is_some(),
        }))
    }
}

#[tokio::test]
async fn runtime_aware_tool_adapter_default_runtime() {
    let tool = Arc::new(EchoRuntimeTool);
    let adapter = RuntimeAwareToolAdapter::new(tool);

    let result = adapter.call(json!({"key": "value"})).await.unwrap();
    assert_eq!(result["tool_call_id"], "");
    assert_eq!(result["has_store"], false);
    assert_eq!(result["has_state"], false);
    assert_eq!(result["args"]["key"], "value");
}

#[tokio::test]
async fn runtime_aware_tool_adapter_with_runtime() {
    let tool = Arc::new(EchoRuntimeTool);
    let adapter = RuntimeAwareToolAdapter::new(tool);

    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: Some(json!({"counter": 42})),
        tool_call_id: "call-123".into(),
        config: None,
    };
    adapter.set_runtime(runtime).await;

    let result = adapter.call(json!({})).await.unwrap();
    assert_eq!(result["tool_call_id"], "call-123");
    assert_eq!(result["has_state"], true);
}

#[test]
fn runtime_aware_tool_as_tool_definition() {
    let tool = EchoRuntimeTool;
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "echo_runtime");
    assert_eq!(def.description, "Echoes runtime info");
    assert!(def.extras.is_none());
}

#[test]
fn tool_runtime_fields() {
    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: Some(json!({"key": "val"})),
        tool_call_id: "tc-1".into(),
        config: Some(RunnableConfig::default().with_run_name("test")),
    };
    assert_eq!(runtime.tool_call_id, "tc-1");
    assert!(runtime.state.is_some());
    assert_eq!(
        runtime.config.as_ref().unwrap().run_name.as_deref(),
        Some("test")
    );
}

#[test]
fn runtime_default_fields() {
    let runtime = Runtime {
        store: None,
        stream_writer: None,
    };
    assert!(runtime.store.is_none());
    assert!(runtime.stream_writer.is_none());
}

#[test]
fn runnable_config_builder_chain() {
    let config = RunnableConfig::default()
        .with_tags(vec!["tag1".into(), "tag2".into()])
        .with_run_name("my-run")
        .with_run_id("run-123")
        .with_max_concurrency(4)
        .with_recursion_limit(25)
        .with_metadata("key", json!("value"));

    assert_eq!(config.tags, vec!["tag1", "tag2"]);
    assert_eq!(config.run_name.as_deref(), Some("my-run"));
    assert_eq!(config.run_id.as_deref(), Some("run-123"));
    assert_eq!(config.max_concurrency, Some(4));
    assert_eq!(config.recursion_limit, Some(25));
    assert_eq!(config.metadata["key"], json!("value"));
}

#[test]
fn runnable_config_serde_roundtrip() {
    let config = RunnableConfig::default()
        .with_run_name("test")
        .with_metadata("version", json!(1));
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: RunnableConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.run_name.as_deref(), Some("test"));
    assert_eq!(deserialized.metadata["version"], json!(1));
}
