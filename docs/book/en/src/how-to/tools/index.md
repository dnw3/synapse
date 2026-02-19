# Tools

Tools give LLMs the ability to take actions in the world -- calling APIs, querying databases, performing calculations, or any other side effect. Synaptic provides a complete tool system built around the `Tool` trait defined in `synaptic-core`.

## Key Components

| Component | Crate | Description |
|-----------|-------|-------------|
| `Tool` trait | `synaptic-core` | The interface every tool must implement: `name()`, `description()`, and `call()` |
| `ToolRegistry` | `synaptic-tools` | Thread-safe collection of registered tools (`Arc<RwLock<HashMap>>`) |
| `SerialToolExecutor` | `synaptic-tools` | Dispatches tool calls by name through the registry |
| `ToolNode` | `synaptic-graph` | Graph node that executes tool calls from AI messages in a state machine workflow |
| `ToolDefinition` | `synaptic-core` | Schema description sent to the model so it knows what tools are available |
| `ToolChoice` | `synaptic-core` | Controls whether and how the model selects tools |

## How It Works

1. You define tools by implementing the `Tool` trait.
2. Register them in a `ToolRegistry`.
3. Convert them to `ToolDefinition` values and attach them to a `ChatRequest` so the model knows what tools are available.
4. When the model responds with `ToolCall` entries, dispatch them through `SerialToolExecutor` to get results.
5. Send the results back to the model as `Message::tool(...)` messages to continue the conversation.

## Quick Example

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

## Sub-Pages

- [Custom Tools](custom-tool.md) -- implement the `Tool` trait for your own tools
- [Tool Registry](registry.md) -- register, look up, and execute tools
- [Tool Choice](tool-choice.md) -- control how the model selects tools with `ToolChoice`
- [Tool Definition Extras](tool-extras.md) -- attach provider-specific parameters to tool definitions
- [Runtime-Aware Tools](runtime-aware.md) -- tools that access graph state, store, and runtime context
