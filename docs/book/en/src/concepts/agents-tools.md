# Agents & Tools

Agents are systems where an LLM decides what actions to take. Rather than following a fixed script, the model examines the conversation, chooses which tools to call (if any), processes the results, and decides whether to call more tools or produce a final answer. This page explains how Synaptic models tools, how they are registered and executed, and how the agent loop works.

## The Tool Trait

A tool in Synaptic is anything that implements the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn call(&self, args: Value) -> Result<Value, SynapticError>;
}
```

- `name()` returns a unique identifier the LLM uses to refer to this tool.
- `description()` explains what the tool does, in natural language. This is sent to the LLM so it knows when and how to use the tool.
- `call()` executes the tool with JSON arguments and returns a JSON result.

The trait is intentionally minimal. A tool does not know about conversations, memory, or models. It receives arguments, does work, and returns a result. This keeps tools reusable and testable in isolation.

## ToolDefinition

When tools are sent to an LLM, they are described as `ToolDefinition` structs:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,  // JSON Schema
    pub extras: Option<HashMap<String, Value>>,  // provider-specific params
}
```

The `parameters` field is a JSON Schema that describes the tool's expected arguments. LLM providers use this schema to generate valid tool calls. The `ToolDefinition` is metadata about the tool -- it never executes anything.

The optional `extras` field carries provider-specific parameters (e.g., Anthropic's `cache_control`). Provider adapters in `synaptic-models` forward these to the API when present.

## ToolCall and ToolChoice

When an LLM decides to use a tool, it produces a `ToolCall`:

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
```

The `id` links the call to its result. When a tool finishes execution, the result is wrapped in a `Message::tool(result, tool_call_id)` that references this ID, allowing the LLM to match results back to calls.

`ToolChoice` controls the LLM's tool-calling behavior:

| Variant | Behavior |
|---------|----------|
| `Auto` | The model decides whether to call tools |
| `Required` | The model must call at least one tool |
| `None` | Tool calling is disabled |
| `Specific(name)` | The model must call the named tool |

`ToolChoice` is set on `ChatRequest` via `.with_tool_choice()`.

## ToolRegistry

The `ToolRegistry` is a thread-safe collection of tools, backed by `Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>`:

```rust
use synaptic::tools::ToolRegistry;

let registry = ToolRegistry::new();
registry.register(Arc::new(WeatherTool))?;
registry.register(Arc::new(CalculatorTool))?;

// Look up a tool by name
let tool = registry.get("weather");
```

Registration is idempotent -- registering a tool with the same name replaces the previous one. The `Arc<RwLock<_>>` ensures safe concurrent access: multiple readers can look up tools simultaneously, and registration briefly acquires a write lock.

## Tool Executors

Executors bridge the gap between tool calls from an LLM and the tool registry:

**`SerialToolExecutor`** -- executes tool calls one at a time. Simple and predictable:

```rust
let executor = SerialToolExecutor::new(registry);
let result = executor.execute("weather", json!({"city": "Tokyo"})).await?;
```

**`ParallelToolExecutor`** -- executes multiple tool calls concurrently. Useful when the LLM produces several independent tool calls in a single response.

## Tool Wrappers

Synaptic provides wrapper types that add behavior to existing tools:

- **`HandleErrorTool`** -- catches errors from the inner tool and returns them as a string result instead of propagating the error. This allows the LLM to see the error and retry with different arguments.
- **`ReturnDirectTool`** -- marks the tool's output as the final response, short-circuiting the agent loop instead of feeding the result back to the LLM.

## ToolNode

In the graph system, `ToolNode` is a pre-built graph node that processes AI messages containing tool calls. It:

1. Reads the last message from the graph state
2. Extracts all `ToolCall` entries from it
3. Executes each tool call via a `SerialToolExecutor`
4. Appends the results as `Message::tool(...)` messages back to the state

`ToolNode` is the standard way to handle tool execution inside a graph workflow. You do not need to write tool dispatching logic yourself.

## The ReAct Agent Pattern

ReAct (Reasoning + Acting) is the most common agent pattern. The model alternates between reasoning about what to do and acting by calling tools. Synaptic provides a prebuilt ReAct agent via `create_react_agent()`:

```rust
use synaptic::graph::{create_react_agent, MessageState};

let graph = create_react_agent(model, tools)?;
let state = MessageState::from_messages(vec![
    Message::human("What is the weather in Tokyo?"),
]);
let result = graph.invoke(state).await?;
```

This builds a graph with two nodes:

```
[START] --> [agent] --tool_calls--> [tools] --> [agent] ...
                   \--no_tools----> [END]
```

- **"agent" node**: Calls the LLM with the current messages and tool definitions. The LLM's response is appended to the state.
- **"tools" node**: A `ToolNode` that executes any tool calls from the agent's response and appends results.

The conditional edge after "agent" checks if the last message has tool calls. If yes, route to "tools". If no, route to END. The edge from "tools" always returns to "agent", creating the loop.

### The Agent Loop in Detail

1. The user message enters the graph state.
2. The "agent" node sends all messages to the LLM along with tool definitions.
3. The LLM responds. If it includes tool calls:
   a. The response (with tool calls) is appended to the state.
   b. Routing sends execution to the "tools" node.
   c. Each tool call is executed and results are appended as Tool messages.
   d. Routing sends execution back to the "agent" node.
   e. The LLM now sees the tool results and can decide what to do next.
4. When the LLM responds without tool calls, it has produced its final answer. Routing sends execution to END.

This loop continues until the LLM decides it has enough information to answer directly, or until the graph's iteration safety limit (100) is reached.

## ReactAgentOptions

The `create_react_agent_with_options()` function accepts a `ReactAgentOptions` struct for advanced configuration:

```rust
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let options = ReactAgentOptions {
    checkpointer: Some(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())))),
    system_prompt: Some("You are a helpful weather assistant.".into()),
    interrupt_before: vec!["tools".into()],
    interrupt_after: vec![],
};

let graph = create_react_agent_with_options(model, tools, options)?;
```

| Option | Purpose |
|--------|---------|
| `checkpointer` | State persistence for resumption across invocations |
| `system_prompt` | Prepended to messages before each LLM call |
| `interrupt_before` | Pause before named nodes (for human approval of tool calls) |
| `interrupt_after` | Pause after named nodes (for human review of tool results) |

Setting `interrupt_before: vec!["tools".into()]` creates a human-in-the-loop agent: the graph pauses before executing tools, allowing a human to inspect the proposed tool calls, modify them, or reject them entirely. The graph is then resumed via `update_state()`.

## See Also

- [Custom Tools](../how-to/tools/custom-tool.md) -- creating tools with the `#[tool]` macro
- [Tool Registry](../how-to/tools/registry.md) -- managing tool collections
- [Tool Choice](../how-to/tools/tool-choice.md) -- controlling model tool-calling behavior
- [Tool Definition Extras](../how-to/tools/tool-extras.md) -- provider-specific parameters
- [Runtime-Aware Tools](../how-to/tools/runtime-aware.md) -- tools with store/state access
- [Tool Node](../how-to/graph/tool-node.md) -- ToolNode in graph workflows
- [Graph](graph.md) -- the graph system that agents run within
