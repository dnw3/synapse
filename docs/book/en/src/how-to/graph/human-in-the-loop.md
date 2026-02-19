# Human-in-the-Loop

Human-in-the-loop (HITL) allows you to pause graph execution at specific points, giving a human the opportunity to review, approve, or modify the state before the graph continues. Synaptic supports two approaches:

1. **`interrupt_before` / `interrupt_after`** -- declarative interrupts on the `StateGraph` builder.
2. **`interrupt()` function** -- programmatic interrupts inside nodes via `Command`.

Both require a checkpointer to persist state for later resumption.

## Interrupt Before and After

The `StateGraph` builder provides two interrupt modes:

- **`interrupt_before(nodes)`** -- pause execution **before** the named nodes run.
- **`interrupt_after(nodes)`** -- pause execution **after** the named nodes run.

### Example: Approval Before Tool Execution

A common pattern is to interrupt before a tool execution node so a human can review the tool calls the agent proposed:

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, MemorySaver, CheckpointConfig, END};
use synaptic_core::Message;
use std::sync::Arc;

let agent_node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("I want to call the delete_file tool."));
    Ok(state.into())
});

let tool_node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::tool("File deleted.", "call-1"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_edge("agent", "tools")
    .add_edge("tools", END)
    // Pause before the tools node executes
    .interrupt_before(vec!["tools".to_string()])
    .compile()?
    .with_checkpointer(Arc::new(MemorySaver::new()));

let config = CheckpointConfig::new("thread-1");
let initial = MessageState::with_messages(vec![Message::human("Delete old logs")]);
```

### Step 1: First Invocation -- Interrupt

The first `invoke_with_config()` runs the `agent` node, then stops before `tools`:

```rust
let result = graph.invoke_with_config(initial, Some(config.clone())).await?;

// Returns GraphResult::Interrupted
assert!(result.is_interrupted());

// You can inspect the interrupt value
if let Some(iv) = result.interrupt_value() {
    println!("Interrupted: {iv}");
}
```

At this point, the checkpointer has saved the state after `agent` ran, with `tools` as the next node.

### Step 2: Human Review

The human can inspect the saved state to review what the agent proposed:

```rust
if let Some(state) = graph.get_state(&config).await? {
    for msg in &state.messages {
        println!("[{}] {}", msg.role(), msg.content());
    }
}
```

### Step 3: Update State (Optional)

If the human wants to modify the state before resuming -- for example, to add an approval message or to change the tool call -- use `update_state()`:

```rust
let approval = MessageState::with_messages(vec![
    Message::human("Approved -- go ahead and delete."),
]);

graph.update_state(&config, approval).await?;
```

`update_state()` loads the current checkpoint, calls `State::merge()` with the provided update, and saves the merged result back to the checkpointer.

### Step 4: Resume Execution

Resume the graph by calling `invoke_with_config()` again with the same config and a default (empty) state. The graph loads the checkpoint and continues from the interrupted node:

```rust
let result = graph
    .invoke_with_config(MessageState::default(), Some(config))
    .await?;

// The graph executed "tools" and reached END
let state = result.into_state();
println!("Final messages: {}", state.messages.len());
```

## Programmatic Interrupt with `interrupt()`

For more control, nodes can call the `interrupt()` function to pause execution with a custom value. This is useful when the decision to interrupt depends on runtime state:

```rust
use synaptic_graph::{interrupt, Node, NodeOutput, MessageState};

struct ApprovalNode;

#[async_trait]
impl Node<MessageState> for ApprovalNode {
    async fn process(&self, state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
        // Check if any tool call is potentially dangerous
        if let Some(msg) = state.last_message() {
            for call in msg.tool_calls() {
                if call.name == "delete_file" {
                    // Interrupt and ask for approval
                    return Ok(interrupt(serde_json::json!({
                        "question": "Approve file deletion?",
                        "tool_call": call.name,
                    })));
                }
            }
        }
        // No dangerous calls -- continue normally
        Ok(state.into())
    }
}
```

The caller receives a `GraphResult::Interrupted` with the interrupt value:

```rust
let result = graph.invoke_with_config(state, Some(config.clone())).await?;
if result.is_interrupted() {
    let question = result.interrupt_value().unwrap();
    println!("Agent asks: {}", question["question"]);
}
```

## Dynamic Routing with `Command`

Nodes can also use `Command` to override the normal edge-based routing:

```rust
use synaptic_graph::{Command, NodeOutput};

// Route to a specific node, skipping normal edges
Ok(NodeOutput::Command(Command::goto("summary")))

// Route to a specific node with a state update
Ok(NodeOutput::Command(Command::goto_with_update("next", delta_state)))

// End the graph immediately
Ok(NodeOutput::Command(Command::end()))

// Update state without overriding routing
Ok(NodeOutput::Command(Command::update(delta_state)))
```

## `interrupt_after`

`interrupt_after` works the same way, but the specified node runs **before** the interrupt. This is useful when you want to see the node's output before deciding whether to continue:

```rust
let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_edge("agent", "tools")
    .add_edge("tools", END)
    // Interrupt after the agent node runs (to review its output)
    .interrupt_after(vec!["agent".to_string()])
    .compile()?
    .with_checkpointer(Arc::new(MemorySaver::new()));
```

## `GraphResult`

`graph.invoke()` returns `Result<GraphResult<S>, SynapticError>`. `GraphResult` is an enum:

- **`GraphResult::Complete(state)`** -- graph ran to `END` normally.
- **`GraphResult::Interrupted { state, interrupt_value }`** -- graph paused.

Key methods:

| Method | Description |
|--------|-------------|
| `is_complete()` | Returns `true` if the graph completed normally |
| `is_interrupted()` | Returns `true` if the graph was interrupted |
| `state()` | Borrow the state (regardless of completion/interrupt) |
| `into_state()` | Consume and return the state |
| `interrupt_value()` | Returns `Some(&Value)` if interrupted, `None` otherwise |

## Notes

- Interrupts require a checkpointer. Without one, the graph cannot save state for resumption.
- `interrupt_before` / `interrupt_after` return `GraphResult::Interrupted` (not an error).
- Programmatic `interrupt()` also returns `GraphResult::Interrupted` with the value you pass.
- You can interrupt at multiple nodes by passing multiple names to `interrupt_before()` or `interrupt_after()`.
- You can combine `interrupt_before` and `interrupt_after` on different nodes in the same graph.
