# State & Nodes

Graphs in Synaptic operate on a **state** value that flows through **nodes**. Each node receives the current state, processes it, and returns an updated state. The `State` trait defines how states are merged, and the `Node<S>` trait defines how nodes process state.

## The `State` Trait

Any type used as graph state must implement the `State` trait:

```rust
pub trait State: Clone + Send + Sync + 'static {
    /// Merge another state into this one (reducer pattern).
    fn merge(&mut self, other: Self);
}
```

The `merge()` method is called when combining state updates -- for example, when `update_state()` is used during human-in-the-loop flows. The merge semantics are up to you: append, replace, or any custom logic.

## `MessageState` -- The Built-in State

For the common case of conversational agents, Synaptic provides `MessageState`:

```rust
use synaptic_graph::MessageState;
use synaptic_core::Message;

// Create an empty state
let state = MessageState::new();

// Create with initial messages
let state = MessageState::with_messages(vec![
    Message::human("Hello"),
    Message::ai("Hi there!"),
]);

// Access the last message
if let Some(msg) = state.last_message() {
    println!("Last: {}", msg.content());
}
```

`MessageState` implements `State` by appending messages on merge:

```rust
fn merge(&mut self, other: Self) {
    self.messages.extend(other.messages);
}
```

This append-only behavior is the right default for conversational workflows where each node adds new messages to the history.

## Custom State

You can define your own state type for non-conversational graphs:

```rust
use synaptic_graph::State;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PipelineState {
    input: String,
    steps_completed: Vec<String>,
    result: Option<String>,
}

impl State for PipelineState {
    fn merge(&mut self, other: Self) {
        self.steps_completed.extend(other.steps_completed);
        if other.result.is_some() {
            self.result = other.result;
        }
    }
}
```

If you plan to use checkpointing, your state must also implement `Serialize` and `Deserialize`.

## The `Node<S>` Trait

A node is any type that implements `Node<S>`:

```rust
use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_graph::{Node, NodeOutput, MessageState};
use synaptic_core::Message;

struct GreeterNode;

#[async_trait]
impl Node<MessageState> for GreeterNode {
    async fn process(&self, mut state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
        state.messages.push(Message::ai("Hello! How can I help?"));
        Ok(state.into()) // NodeOutput::State(state)
    }
}
```

Nodes return `NodeOutput<S>`, which is an enum:

- **`NodeOutput::State(S)`** -- a regular state update (existing behavior). The `From<S>` impl lets you write `Ok(state.into())`.
- **`NodeOutput::Command(Command<S>)`** -- a control flow command (goto, interrupt, fan-out). See [Human-in-the-Loop](human-in-the-loop.md) for interrupt examples.

Nodes are `Send + Sync`, so they can safely hold shared references (e.g., `Arc<dyn ChatModel>`) and be used across async tasks.

## `FnNode` -- Closure-based Nodes

For simple logic, `FnNode` wraps an async closure as a node without defining a separate struct:

```rust
use synaptic_graph::{FnNode, MessageState};
use synaptic_core::Message;

let greeter = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Hello from a closure!"));
    Ok(state.into())
});
```

`FnNode` accepts any function with the signature `Fn(S) -> Future<Output = Result<NodeOutput<S>, SynapticError>>` where `S: State`.

## Adding Nodes to a Graph

Nodes are added to a `StateGraph` with a string name. The name is used to reference the node in edges and conditional routing:

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, END};
use synaptic_core::Message;

let node_a = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step A"));
    Ok(state.into())
});

let node_b = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step B"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("a", node_a)
    .add_node("b", node_b)
    .set_entry_point("a")
    .add_edge("a", "b")
    .add_edge("b", END)
    .compile()?;
```

Both struct-based nodes (implementing `Node<S>`) and `FnNode` closures can be passed to `add_node()` interchangeably.
