# Command & Routing

`Command<S>` gives nodes dynamic control over graph execution, allowing them to override edge-based routing, update state, fan out to multiple nodes, or terminate early. Use it when routing decisions depend on runtime state.

Nodes return `NodeOutput<S>` -- either `NodeOutput::State(S)` for a regular state update (via `Ok(state.into())`), or `NodeOutput::Command(Command<S>)` for dynamic control flow.

## Command Constructors

| Constructor | Behavior |
|-------------|----------|
| `Command::goto("node")` | Route to a specific node, skipping normal edges |
| `Command::goto_with_update("node", delta)` | Route to a node and merge `delta` into state |
| `Command::update(delta)` | Merge `delta` into state, then follow normal routing |
| `Command::end()` | Terminate the graph immediately |
| `Command::send(targets)` | Fan-out to multiple nodes via [`Send`] |
| `Command::resume(value)` | Resume from a previous interrupt (see [Interrupt & Resume](interrupt-resume.md)) |

## Conditional Routing with `goto`

A "triage" node inspects the input and routes to different handlers:

```rust,ignore
use synaptic_graph::{Command, FnNode, NodeOutput, State, StateGraph, END};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TicketState {
    category: String,
    resolved: bool,
}

impl State for TicketState {
    fn merge(&mut self, other: Self) {
        if !other.category.is_empty() { self.category = other.category; }
        self.resolved = self.resolved || other.resolved;
    }
}

let triage = FnNode::new(|state: TicketState| async move {
    let target = if state.category == "billing" {
        "billing_handler"
    } else {
        "support_handler"
    };
    Ok(NodeOutput::Command(Command::goto(target)))
});

let billing = FnNode::new(|mut state: TicketState| async move {
    state.resolved = true;
    Ok(state.into())
});

let support = FnNode::new(|mut state: TicketState| async move {
    state.resolved = true;
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("triage", triage)
    .add_node("billing_handler", billing)
    .add_node("support_handler", support)
    .set_entry_point("triage")
    .add_edge("billing_handler", END)
    .add_edge("support_handler", END)
    .compile()?;

let result = graph.invoke(TicketState {
    category: "billing".into(),
    resolved: false,
}).await?.into_state();
assert!(result.resolved);
```

## Routing with State Update

`goto_with_update` routes and merges a state delta in one step. The delta is merged via `State::merge()` before the target node runs:

```rust,ignore
Ok(NodeOutput::Command(Command::goto_with_update("escalation", delta)))
```

## Update Without Routing

`Command::update(delta)` merges state but follows normal edges. Useful when a node contributes a partial update without overriding the next step:

```rust,ignore
Ok(NodeOutput::Command(Command::update(delta)))
```

## Early Termination

`Command::end()` stops the graph immediately. No further nodes execute:

```rust,ignore
let guard = FnNode::new(|state: TicketState| async move {
    if state.category == "spam" {
        return Ok(NodeOutput::Command(Command::end()));
    }
    Ok(state.into())
});
```

## Fan-Out with `Send`

`Command::send()` dispatches work to multiple targets. Each `Send` carries a node name and a JSON payload:

```rust,ignore
use synaptic_graph::Send;

let targets = vec![
    Send::new("worker", serde_json::json!({"chunk": "part1"})),
    Send::new("worker", serde_json::json!({"chunk": "part2"})),
];
Ok(NodeOutput::Command(Command::send(targets)))
```

> **Note:** Full parallel fan-out is not yet implemented. Targets are currently processed sequentially.

## Commands in Streaming Mode

Commands work identically when streaming. If node "a" issues `Command::goto("c")`, the stream yields events for "a" and "c" but skips "b", even if an `a -> b` edge exists.
