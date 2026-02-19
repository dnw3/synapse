# Interrupt & Resume

`interrupt(value)` pauses graph execution and returns control to the caller with a JSON value, enabling human-in-the-loop workflows where a node decides at runtime whether to pause. A checkpointer is required to persist state for later resumption.

For declarative interrupts (`interrupt_before`/`interrupt_after`), see [Human-in-the-Loop](human-in-the-loop.md).

## The `interrupt()` Function

```rust,ignore
use synaptic_graph::{interrupt, Node, NodeOutput, MessageState};
use synaptic_core::SynapticError;
use async_trait::async_trait;

struct ApprovalGate;

#[async_trait]
impl Node<MessageState> for ApprovalGate {
    async fn process(
        &self,
        state: MessageState,
    ) -> Result<NodeOutput<MessageState>, SynapticError> {
        if let Some(msg) = state.last_message() {
            for call in msg.tool_calls() {
                if call.name == "delete_database" {
                    return Ok(interrupt(serde_json::json!({
                        "question": "Approve database deletion?",
                        "tool_call": call.name,
                    })));
                }
            }
        }
        Ok(state.into()) // continue normally
    }
}
```

## Detecting Interrupts with `GraphResult`

`graph.invoke()` returns `GraphResult<S>` -- either `Complete(state)` or `Interrupted { state, interrupt_value }`:

```rust,ignore
let result = graph.invoke_with_config(state, Some(config.clone())).await?;

if result.is_interrupted() {
    println!("Paused: {}", result.interrupt_value().unwrap());
} else {
    println!("Done: {:?}", result.into_state());
}
```

## Full Round-Trip Example

```rust,ignore
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use serde_json::json;
use synaptic_graph::{
    interrupt, CheckpointConfig, FnNode, MemorySaver,
    NodeOutput, State, StateGraph, END,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ReviewState {
    proposal: String,
    approved: bool,
    done: bool,
}

impl State for ReviewState {
    fn merge(&mut self, other: Self) {
        if !other.proposal.is_empty() { self.proposal = other.proposal; }
        self.approved = self.approved || other.approved;
        self.done = self.done || other.done;
    }
}

let propose = FnNode::new(|mut state: ReviewState| async move {
    state.proposal = "Delete all temporary files".into();
    Ok(state.into())
});

let gate = FnNode::new(|state: ReviewState| async move {
    Ok(interrupt(json!({"question": "Approve?", "proposal": state.proposal})))
});

let execute = FnNode::new(|mut state: ReviewState| async move {
    state.done = true;
    Ok(state.into())
});

let saver = Arc::new(MemorySaver::new());
let graph = StateGraph::new()
    .add_node("propose", propose)
    .add_node("gate", gate)
    .add_node("execute", execute)
    .set_entry_point("propose")
    .add_edge("propose", "gate")
    .add_edge("gate", "execute")
    .add_edge("execute", END)
    .compile()?
    .with_checkpointer(saver);

let config = CheckpointConfig::new("review-thread");

// Step 1: Invoke -- graph pauses at the gate
let result = graph
    .invoke_with_config(ReviewState::default(), Some(config.clone()))
    .await?;
assert!(result.is_interrupted());

// Step 2: Review saved state
let saved = graph.get_state(&config).await?.unwrap();
println!("Proposal: {}", saved.proposal);

// Step 3: Optionally update state before resuming
graph.update_state(&config, ReviewState {
    proposal: String::new(), approved: true, done: false,
}).await?;

// Step 4: Resume execution
let result = graph
    .invoke_with_config(ReviewState::default(), Some(config))
    .await?;
assert!(result.is_complete());
assert!(result.into_state().done);
```

## Notes

- **Checkpointer required.** Without one, state cannot be saved between interrupt and resume. `MemorySaver` works for development; implement `Checkpointer` for production.
- **State is not merged on interrupt.** When a node returns `interrupt()`, the node's state update is not applied -- only state from previously executed nodes is preserved.
- **`Command::resume(value)`** passes a value to the graph on resumption, available via the command's `resume_value` field.
- **State history.** Call `graph.get_state_history(&config)` to inspect all checkpoints for a thread.
