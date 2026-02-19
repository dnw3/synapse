# Graph

Synaptic provides LangGraph-style graph orchestration through the `synaptic_graph` crate. A `StateGraph` is a state machine where **nodes** process state and **edges** control the flow between nodes. This architecture supports fixed routing, conditional branching, checkpointing for persistence, human-in-the-loop interrupts, and streaming execution.

## Core Concepts

| Concept | Description |
|---------|-------------|
| `State` trait | Defines how graph state is merged when nodes produce updates |
| `Node<S>` trait | A processing unit that takes state and returns updated state |
| `StateGraph` | Builder for assembling nodes and edges into a graph |
| `CompiledGraph` | The executable graph produced by `StateGraph::compile()` |
| `Checkpointer` | Trait for persisting graph state across invocations |
| `ToolNode` | Prebuilt node that auto-dispatches tool calls from AI messages |

## How It Works

1. Define a state type that implements `State` (or use the built-in `MessageState`).
2. Create nodes -- either by implementing the `Node<S>` trait or by wrapping a closure with `FnNode`.
3. Build a graph with `StateGraph::new()`, adding nodes and edges.
4. Call `.compile()` to validate the graph and produce a `CompiledGraph`.
5. Run the graph with `invoke()` for a single result or `stream()` for per-node events.

```rust
use synaptic_graph::{StateGraph, MessageState, FnNode, END};
use synaptic_core::Message;

let greet = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Hello from the graph!"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("greet", greet)
    .set_entry_point("greet")
    .add_edge("greet", END)
    .compile()?;

let initial = MessageState::with_messages(vec![Message::human("Hi")]);
let result = graph.invoke(initial).await?;
assert_eq!(result.messages.len(), 2);
```

## Guides

- [State & Nodes](state-nodes.md) -- define state types and processing nodes
- [Edges](edges.md) -- connect nodes with fixed and conditional edges
- [Graph Streaming](streaming.md) -- consume per-node events during execution (single and multi-mode)
- [Checkpointing](checkpointing.md) -- persist and resume graph state
- [Human-in-the-Loop](human-in-the-loop.md) -- interrupt execution for human review
- [Tool Node](tool-node.md) -- auto-dispatch tool calls from AI messages
- [Visualization](visualization.md) -- render graphs as Mermaid, ASCII, DOT, or PNG

## Advanced Features

### Node Caching

Use `add_node_with_cache()` to cache node results based on input state. Cached entries expire after the specified TTL:

```rust
use synaptic_graph::{StateGraph, CachePolicy, END};
use std::time::Duration;

let graph = StateGraph::new()
    .add_node_with_cache(
        "expensive",
        expensive_node,
        CachePolicy::new(Duration::from_secs(300)),
    )
    .add_edge("expensive", END)
    .set_entry_point("expensive")
    .compile()?;
```

When the same input state is seen again within the TTL, the cached result is returned without re-executing the node.

### Deferred Nodes

Use `add_deferred_node()` to create nodes that wait for ALL incoming paths to complete before executing. This is useful for fan-in aggregation after parallel fan-out with `Send`:

```rust
let graph = StateGraph::new()
    .add_node("branch_a", node_a)
    .add_node("branch_b", node_b)
    .add_deferred_node("aggregate", aggregator_node)
    .add_edge("branch_a", "aggregate")
    .add_edge("branch_b", "aggregate")
    .add_edge("aggregate", END)
    .set_entry_point("branch_a")
    .compile()?;
```

### Structured Output (response_format)

When creating an agent with `create_agent()`, set `response_format` in `AgentOptions` to force the final response into a specific JSON schema:

```rust
use synaptic_graph::{create_agent, AgentOptions};

let graph = create_agent(model, tools, AgentOptions {
    response_format: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" },
            "confidence": { "type": "number" }
        },
        "required": ["answer", "confidence"]
    })),
    ..Default::default()
})?;
```

When the agent produces its final answer (no tool calls), it re-calls the model with structured output instructions matching the schema.
