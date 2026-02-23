# Graph

LCEL chains are powerful for linear pipelines, but some workflows need cycles, conditional branching, checkpointed state, and human intervention. The graph system (Synaptic's equivalent of LangGraph) provides these capabilities through a state-machine abstraction. This page explains the graph model, its key concepts, and how it differs from chain-based composition.

## Why Graphs?

Consider a ReAct agent. The LLM calls tools, sees the results, and decides whether to call more tools or produce a final answer. This is a loop -- the execution path is not known in advance. LCEL chains compose linearly (A | B | C), but a ReAct agent needs to go from A to B, then back to A, then conditionally to C.

Graphs solve this. Each step is a **node**, transitions are **edges**, and the graph runtime handles routing, checkpointing, and streaming. The execution path emerges at runtime based on the state.

## State

Every graph operates on a shared state type that implements the `State` trait:

```rust
pub trait State: Send + Sync + Clone + 'static {
    fn merge(&mut self, other: Self);
}
```

The `merge()` method defines how state updates are combined. When a node returns a new state, it is merged into the current state. This is the graph's "reducer" -- it determines how concurrent or sequential updates compose.

### MessageState

Synaptic provides `MessageState` as the built-in state type for conversational agents:

```rust
pub struct MessageState {
    pub messages: Vec<Message>,
}
```

Its `merge()` implementation appends new messages to the existing list. This means each node can add messages (LLM responses, tool results, etc.) and they accumulate naturally.

You can define custom state types for non-conversational workflows. Any `Clone + Send + Sync + 'static` type that implements `State` (specifically, the `merge` method) can be used.

## Nodes

A node is a unit of computation within the graph:

```rust
#[async_trait]
pub trait Node<S: State>: Send + Sync {
    async fn process(&self, state: S) -> Result<NodeOutput<S>, SynapticError>;
}
```

A node receives the current state, does work, and returns a `NodeOutput<S>`:

- **`NodeOutput::State(S)`** -- a regular state update. The `From<S>` impl lets you write `Ok(state.into())`.
- **`NodeOutput::Command(Command<S>)`** -- a control flow command: dynamic routing (`Command::goto`), early termination (`Command::end`), or interrupts (`interrupt()`).

`FnNode` wraps an async closure into a node, which is the most common way to define nodes:

```rust
let my_node = FnNode::new(|state: MessageState| async move {
    // Process state, add messages, etc.
    Ok(state.into())
});
```

`ToolNode` is a pre-built node that extracts tool calls from the last AI message, executes them, and appends the results. The `tools_condition` function provides standard routing: returns `"tools"` if the last message has tool calls, else `END`.

## Building a Graph

`StateGraph<S>` is the builder:

```rust
use synaptic::graph::{StateGraph, MessageState, END};

let graph = StateGraph::new()
    .add_node("step_1", node_1)
    .add_node("step_2", node_2)
    .set_entry_point("step_1")
    .add_edge("step_1", "step_2")
    .add_edge("step_2", END)
    .compile()?;
```

### add_node(name, node)

Registers a named node. Names are arbitrary strings. Two special constants exist: `START` (the entry sentinel) and `END` (the exit sentinel). You never add `START` or `END` as nodes -- they are implicit.

### set_entry_point(name)

Defines which node executes first after `START`.

### add_edge(source, target)

A fixed edge -- after `source` completes, always go to `target`. The target can be `END` to terminate the graph.

### add_conditional_edges(source, router_fn)

A conditional edge -- after `source` completes, call `router_fn` with the current state to determine the next node:

```rust
.add_conditional_edges("agent", |state: &MessageState| {
    if state.last_message().map_or(false, |m| !m.tool_calls().is_empty()) {
        "tools".to_string()
    } else {
        END.to_string()
    }
})
```

The router function receives a reference to the state and returns the name of the next node (or `END`).

There is also `add_conditional_edges_with_path_map()`, which additionally provides a mapping from router return values to node names. This path map is used by visualization tools to render the conditional branches.

### compile()

Validates the graph (checks that all referenced nodes exist, that the entry point is set, etc.) and returns a `CompiledGraph<S>`.

## Executing a Graph

`CompiledGraph<S>` provides two execution methods:

### invoke(state)

Runs the graph and returns a `GraphResult<S>`:

```rust
let initial = MessageState::with_messages(vec![Message::human("Hello")]);
let result = graph.invoke(initial).await?;

match result {
    GraphResult::Complete(state) => println!("Done: {} messages", state.messages.len()),
    GraphResult::Interrupted { state, interrupt_value } => {
        println!("Paused: {interrupt_value}");
    }
}

// Or use convenience methods:
let state = result.into_state(); // works for both Complete and Interrupted
```

### stream(state, mode)

Returns a `GraphStream` that yields `GraphEvent<S>` after each node executes:

```rust
use futures::StreamExt;
use synaptic::graph::StreamMode;

let mut stream = graph.stream(initial, StreamMode::Values);
while let Some(event) = stream.next().await {
    let event = event?;
    println!("Node '{}' completed", event.node);
}
```

`StreamMode::Values` yields the full state after each node. `StreamMode::Updates` yields the per-node state changes.

## Checkpointing

Graphs support state persistence through the `Checkpointer` trait. After each node executes, the current state and the next scheduled node are saved. This enables:

- **Resumption**: If the process crashes, the graph can resume from the last checkpoint.
- **Human-in-the-loop**: The graph can pause, persist state, and resume later after human input.

`StoreCheckpointer` with an `InMemoryStore` backend is the simplest in-memory checkpointer. For production use, you would implement `Checkpointer` with a database backend.

```rust
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let checkpointer = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
let graph = graph.with_checkpointer(checkpointer);
```

Checkpoints are identified by a `CheckpointConfig` that includes a `thread_id`. Different threads have independent checkpoint histories.

### get_state / get_state_history

You can inspect the current state and full history of a checkpointed graph:

```rust
let current = graph.get_state(&config).await?;
let history = graph.get_state_history(&config).await?;
```

`get_state_history()` returns a list of `(state, next_node)` pairs, ordered from oldest to newest.

## Human-in-the-Loop

Two mechanisms pause graph execution for human intervention:

### interrupt_before(nodes)

The graph pauses **before** executing the named nodes. The current state is checkpointed, and the graph returns `GraphResult::Interrupted`.

```rust
let graph = StateGraph::new()
    // ...
    .interrupt_before(vec!["tools".into()])
    .compile()?;
```

After the interrupt, the human can inspect the state (e.g., review proposed tool calls), modify it via `update_state()`, and resume execution:

```rust
// Inspect the proposed tool calls
let state = graph.get_state(&config).await?.unwrap();

// Modify state if needed
graph.update_state(&config, updated_state).await?;

// Resume execution
let result = graph.invoke_with_config(
    MessageState::default(),
    Some(config),
).await?;
let final_state = result.into_state();
```

### interrupt_after(nodes)

The graph pauses **after** executing the named nodes. The node's output is already in the state, and the next node is recorded in the checkpoint. Useful for reviewing a node's output before proceeding.

### Programmatic interrupt()

Nodes can also interrupt programmatically using the `interrupt()` function:

```rust
use synaptic::graph::{interrupt, NodeOutput};

// Inside a node's process() method:
Ok(interrupt(serde_json::json!({"question": "Approve?"})))
```

This returns `GraphResult::Interrupted` with the specified value, which the caller can inspect via `result.interrupt_value()`.

## Dynamic Control Flow with Command

Nodes can override normal edge-based routing by returning `NodeOutput::Command(...)`:

### Command::goto(target)

Redirects execution to a specific node, skipping normal edge resolution:

```rust
Ok(NodeOutput::Command(Command::goto("summary")))
```

### Command::goto_with_update(target, state_delta)

Routes to a node while also applying a state update:

```rust
Ok(NodeOutput::Command(Command::goto_with_update("next", delta)))
```

### Command::end()

Ends graph execution immediately:

```rust
Ok(NodeOutput::Command(Command::end()))
```

### Command::update(state_delta)

Applies a state update without overriding routing (uses normal edges):

```rust
Ok(NodeOutput::Command(Command::update(delta)))
```

Commands take priority over edges. After a node executes, the graph checks for a command before consulting edges. This enables dynamic, state-dependent control flow that goes beyond what static edge definitions can express.

## Send (Fan-out)

The `Send` mechanism allows a node to dispatch work to multiple target nodes via `Command::send()`, enabling fan-out (map-reduce) patterns within the graph.

## Visualization

`CompiledGraph` provides multiple rendering methods:

| Method | Output | Requirements |
|--------|--------|-------------|
| `draw_mermaid()` | Mermaid flowchart string | None |
| `draw_ascii()` | Plain text summary | None |
| `draw_dot()` | Graphviz DOT format | None |
| `draw_png(path)` | PNG image file | Graphviz `dot` in PATH |
| `draw_mermaid_png(path)` | PNG via mermaid.ink API | Internet access |
| `draw_mermaid_svg(path)` | SVG via mermaid.ink API | Internet access |

`Display` is also implemented, so `println!("{graph}")` outputs the ASCII representation.

Mermaid output example for a ReAct agent:

```
graph TD
    __start__(["__start__"])
    agent["agent"]
    tools["tools"]
    __end__(["__end__"])
    __start__ --> agent
    tools --> agent
    agent -.-> |tools| tools
    agent -.-> |__end__| __end__
```

## Prebuilt Multi-Agent Patterns

Beyond `create_react_agent`, Synaptic provides two multi-agent graph constructors:

### create_supervisor

Builds a supervisor graph where a central LLM orchestrates sub-agents. The supervisor decides which agent to delegate to by calling handoff tools (`transfer_to_<agent_name>`). Each sub-agent is itself a compiled react agent graph.

```rust
use synaptic::graph::{create_supervisor, SupervisorOptions};

let agents = vec![
    ("researcher".to_string(), researcher_graph),
    ("writer".to_string(), writer_graph),
];
let graph = create_supervisor(supervisor_model, agents, SupervisorOptions::default())?;
```

The supervisor loop: supervisor calls LLM → if handoff tool call, route to sub-agent → sub-agent runs to completion → return to supervisor → repeat until supervisor produces a final answer (no tool calls).

### create_swarm

Builds a swarm graph where agents hand off to each other peer-to-peer, without a central coordinator. Each agent has its own model, tools, and system prompt. Handoff is done via `transfer_to_<agent_name>` tool calls.

```rust
use synaptic::graph::{create_swarm, SwarmAgent, SwarmOptions};

let agents = vec![
    SwarmAgent { name: "triage".into(), model, tools, system_prompt: Some("...".into()) },
    SwarmAgent { name: "support".into(), model, tools, system_prompt: Some("...".into()) },
];
let graph = create_swarm(agents, SwarmOptions::default())?;
```

The first agent in the list is the entry point. Each agent runs until it either produces a final answer or hands off to another agent.

## Safety Limits

The graph runtime enforces a maximum of 100 iterations per execution to prevent infinite loops. If a graph cycles more than 100 times, it returns `SynapticError::Graph("max iterations (100) exceeded")`. This is a safety guard, not a configurable limit -- if your workflow legitimately needs more iterations, the graph structure should be reconsidered.

## See Also

- [State & Nodes](../how-to/graph/state-nodes.md) -- building custom nodes and state types
- [Command & Routing](../how-to/graph/command.md) -- dynamic control flow with Command
- [Interrupt & Resume](../how-to/graph/interrupt-resume.md) -- programmatic interrupts
- [Human-in-the-Loop](../how-to/graph/human-in-the-loop.md) -- pausing for human input
- [Streaming](../how-to/graph/streaming.md) -- graph streaming with StreamMode
- [Supervisor](../how-to/multi-agent/supervisor.md) -- supervisor pattern how-to
- [Swarm](../how-to/multi-agent/swarm.md) -- swarm pattern how-to
- [Tool Node](../how-to/graph/tool-node.md) -- ToolNode and tools_condition
