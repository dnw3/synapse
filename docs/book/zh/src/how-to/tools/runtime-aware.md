# Runtime-Aware Tools

`RuntimeAwareTool` 扩展了基本的 `Tool` trait，增加了运行时上下文——当前图状态、Store 引用、流写入器、工具调用 ID 和 Runnable 配置。当工具需要在执行过程中读取或修改图状态时，请实现此 trait。

## `ToolRuntime` 结构体

当 Runtime-Aware Tool 被调用时，它会收到一个包含以下字段的 `ToolRuntime`：

```rust,ignore
pub struct ToolRuntime {
    pub store: Option<Arc<dyn Store>>,
    pub stream_writer: Option<StreamWriter>,
    pub state: Option<Value>,
    pub tool_call_id: String,
    pub config: Option<RunnableConfig>,
}
```

| 字段 | 描述 |
|------|------|
| `store` | 共享的键值存储，用于跨工具持久化 |
| `stream_writer` | 用于在工具内部推送流式输出的写入器 |
| `state` | 当前图状态的序列化快照 |
| `tool_call_id` | 正在执行的工具调用的 ID |
| `config` | 包含标签、元数据和运行 ID 的 Runnable 配置 |

## 实现 `RuntimeAwareTool`

该 trait 要求实现 `name()`、`description()` 和 `call_with_runtime()`。可选择性地覆写 `parameters()` 以提供 JSON schema：

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

## 在图中与 `ToolNode` 配合使用

`ToolNode` 会自动将运行时上下文注入到已注册的 `RuntimeAwareTool` 实例中。使用 `with_runtime_tool()` 注册它们，并可选择性地通过 `with_store()` 附加 Store：

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

当图执行此工具节点并遇到匹配 `"save_note"` 的工具调用时，它会构建一个填充了当前图状态、Store 和工具调用 ID 的 `ToolRuntime`，然后调用 `call_with_runtime()`。

## `RuntimeAwareToolAdapter` -- 在图外使用

如果你需要在期望标准 `Tool` trait 的上下文中使用 `RuntimeAwareTool`（例如，直接与 `SerialToolExecutor` 配合使用），请用 `RuntimeAwareToolAdapter` 包装它：

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

如果在调用 `call()` 之前未调用 `set_runtime()`，适配器会使用默认的空 `ToolRuntime`，所有可选字段设为 `None`，`tool_call_id` 为空字符串。

## 使用 Store 创建 `create_react_agent`

通过 `create_react_agent` 构建 ReAct Agent 时，可以通过 `AgentOptions` 传入 Store，它会自动连接到 `ToolNode`，供所有已注册的 Runtime-Aware Tool 使用：

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
