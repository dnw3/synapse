# Tools

Tool 赋予 LLM 在真实世界中采取行动的能力——调用 API、查询数据库、执行计算或任何其他副作用。Synaptic 围绕 `synaptic-core` 中定义的 `Tool` trait 提供了完整的工具系统。

## 核心组件

| 组件 | Crate | 描述 |
|------|-------|------|
| `Tool` trait | `synaptic-core` | 每个工具必须实现的接口：`name()`、`description()` 和 `call()` |
| `ToolRegistry` | `synaptic-tools` | 线程安全的已注册工具集合（`Arc<RwLock<HashMap>>`） |
| `SerialToolExecutor` | `synaptic-tools` | 通过注册表按名称分发工具调用 |
| `ToolNode` | `synaptic-graph` | 在状态机工作流中执行 AI 消息中工具调用的图节点 |
| `ToolDefinition` | `synaptic-core` | 发送给模型的 schema 描述，让模型知道有哪些可用工具 |
| `ToolChoice` | `synaptic-core` | 控制模型是否以及如何选择工具 |

## 工作原理

1. 通过实现 `Tool` trait 来定义工具。
2. 将工具注册到 `ToolRegistry` 中。
3. 将工具转换为 `ToolDefinition` 值，并附加到 `ChatRequest` 上，让模型知道有哪些可用工具。
4. 当模型返回包含 `ToolCall` 条目的响应时，通过 `SerialToolExecutor` 分发执行以获取结果。
5. 将结果以 `Message::tool(...)` 消息的形式发送回模型，以继续对话。

## 快速示例

```rust
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{Tool, SynapticError};
use synaptic_tools::{ToolRegistry, SerialToolExecutor};

struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &'static str { "add" }
    fn description(&self) -> &'static str { "Add two numbers" }
    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"result": a + b}))
    }
}

let registry = ToolRegistry::new();
registry.register(Arc::new(AddTool))?;

let executor = SerialToolExecutor::new(registry);
let result = executor.execute("add", json!({"a": 3, "b": 4})).await?;
assert_eq!(result, json!({"result": 7.0}));
```

## 子页面

- [自定义工具](custom-tool.md) -- 为你自己的工具实现 `Tool` trait
- [Tool Registry](registry.md) -- 注册、查找和执行工具
- [Tool Choice](tool-choice.md) -- 使用 `ToolChoice` 控制模型如何选择工具
- [Tool Definition Extras](tool-extras.md) -- 将提供商特定参数附加到工具定义
- [Runtime-Aware Tools](runtime-aware.md) -- 能够访问图状态、存储和运行时上下文的工具
