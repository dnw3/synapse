# Runtime-Aware Tools

`RuntimeAwareTool` extends the basic `Tool` trait with runtime context -- current graph state, a store reference, stream writer, tool call ID, and runnable config. Implement this trait for tools that need to read or modify graph state during execution.

## The `ToolRuntime` Struct

When a runtime-aware tool is invoked, it receives a `ToolRuntime` with the following fields:

```rust,ignore
pub struct ToolRuntime {
    pub store: Option<Arc<dyn Store>>,
    pub stream_writer: Option<StreamWriter>,
    pub state: Option<Value>,
    pub tool_call_id: String,
    pub config: Option<RunnableConfig>,
}
```

| Field | Description |
|-------|-------------|
| `store` | Shared key-value store for cross-tool persistence |
| `stream_writer` | Writer for pushing streaming output from within a tool |
| `state` | Serialized snapshot of the current graph state |
| `tool_call_id` | The ID of the tool call being executed |
| `config` | Runnable config with tags, metadata, and run ID |

## Implementing `RuntimeAwareTool`

The trait requires `name()`, `description()`, and `call_with_runtime()`. Optionally override `parameters()` to provide a JSON schema:

```rust,ignore
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{RuntimeAwareTool, ToolRuntime, SynapticError};

struct SaveNoteTool;

#[async_trait]
impl RuntimeAwareTool for SaveNoteTool {
    fn name(&self) -> &'static str { "save_note" }
    fn description(&self) -> &'static str { "Save a note to the store" }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" },
                "text": { "type": "string" }
            },
            "required": ["key", "text"]
        }))
    }

    async fn call_with_runtime(
        &self,
        args: Value,
        runtime: ToolRuntime,
    ) -> Result<Value, SynapticError> {
        let key = args["key"].as_str().unwrap_or("default");
        let text = args["text"].as_str().unwrap_or("");

        if let Some(store) = &runtime.store {
            store.put(
                &["notes"],
                key,
                json!({"text": text}),
            ).await?;
        }

        Ok(json!({"saved": key}))
    }
}
```

## Using with `ToolNode` in a Graph

`ToolNode` automatically injects runtime context into registered `RuntimeAwareTool` instances. Register them with `with_runtime_tool()` and optionally attach a store with `with_store()`:

```rust,ignore
use std::sync::Arc;
use synaptic_graph::ToolNode;
use synaptic_tools::{ToolRegistry, SerialToolExecutor};

let registry = ToolRegistry::new();
let executor = SerialToolExecutor::new(registry);

let save_tool: Arc<dyn RuntimeAwareTool> = Arc::new(SaveNoteTool);

let tool_node = ToolNode::new(executor)
    .with_store(store.clone())
    .with_runtime_tool(save_tool);
```

When the graph executes this tool node and encounters a tool call matching `"save_note"`, it builds a `ToolRuntime` populated with the current graph state, the store, and the tool call ID, then calls `call_with_runtime()`.

## `RuntimeAwareToolAdapter` -- Using Outside a Graph

If you need to use a `RuntimeAwareTool` in a context that expects the standard `Tool` trait (for example, with `SerialToolExecutor` directly), wrap it in a `RuntimeAwareToolAdapter`:

```rust,ignore
use std::sync::Arc;
use synaptic_core::{RuntimeAwareTool, RuntimeAwareToolAdapter, ToolRuntime};

let tool: Arc<dyn RuntimeAwareTool> = Arc::new(SaveNoteTool);
let adapter = RuntimeAwareToolAdapter::new(tool);

// Optionally inject a runtime before calling
adapter.set_runtime(ToolRuntime {
    store: Some(store.clone()),
    stream_writer: None,
    state: None,
    tool_call_id: "call-1".to_string(),
    config: None,
}).await;

// Now use it as a regular Tool
let result = adapter.call(json!({"key": "k", "text": "hello"})).await?;
```

If `set_runtime()` is not called before `call()`, the adapter uses a default empty `ToolRuntime` with all optional fields set to `None` and an empty `tool_call_id`.

## `create_react_agent` with a Store

When building a ReAct agent via `create_react_agent`, pass a store through `AgentOptions` to have it automatically wired into the `ToolNode` for all registered runtime-aware tools:

```rust,ignore
use synaptic_graph::{create_react_agent, AgentOptions};

let graph = create_react_agent(
    model,
    tools,
    AgentOptions {
        store: Some(store),
        ..Default::default()
    },
);
```
