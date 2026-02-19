# Deferred Nodes

`add_deferred_node()` registers a node that is intended to wait until all incoming edges have been traversed before executing. Use deferred nodes as fan-in aggregation points after parallel fan-out with `Command::send()`, where multiple upstream branches must complete before the aggregator runs.

## Adding a Deferred Node

Use `add_deferred_node()` on `StateGraph` instead of `add_node()`:

```rust,ignore
use synaptic_graph::{FnNode, State, StateGraph, END};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AggState { values: Vec<String> }

impl State for AggState {
    fn merge(&mut self, other: Self) { self.values.extend(other.values); }
}

let worker_a = FnNode::new(|mut state: AggState| async move {
    state.values.push("from_a".into());
    Ok(state.into())
});

let worker_b = FnNode::new(|mut state: AggState| async move {
    state.values.push("from_b".into());
    Ok(state.into())
});

let aggregator = FnNode::new(|state: AggState| async move {
    println!("Collected {} results", state.values.len());
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("worker_a", worker_a)
    .add_node("worker_b", worker_b)
    .add_deferred_node("aggregator", aggregator)
    .add_edge("worker_a", "aggregator")
    .add_edge("worker_b", "aggregator")
    .add_edge("aggregator", END)
    .set_entry_point("worker_a")
    .compile()?;
```

## Querying Deferred Status

After compiling, check whether a node is deferred with `is_deferred()`:

```rust,ignore
assert!(graph.is_deferred("aggregator"));
assert!(!graph.is_deferred("worker_a"));
```

## Counting Incoming Edges

`incoming_edge_count()` returns the total number of fixed and conditional edges targeting a node. Use it to validate that a deferred node has the expected number of upstream dependencies:

```rust,ignore
assert_eq!(graph.incoming_edge_count("aggregator"), 2);
assert_eq!(graph.incoming_edge_count("worker_a"), 0);
```

The count includes fixed edges (`add_edge`) and conditional edge path-map entries that reference the node. Conditional edges without a path map are not counted because their targets cannot be determined statically.

## Combining with `Command::send()`

Deferred nodes are designed as the aggregation target after `Command::send()` fans out work:

```rust,ignore
use synaptic_graph::{Command, NodeOutput, Send};

let dispatcher = FnNode::new(|_state: AggState| async move {
    let targets = vec![
        Send::new("worker", serde_json::json!({"chunk": "A"})),
        Send::new("worker", serde_json::json!({"chunk": "B"})),
    ];
    Ok(NodeOutput::Command(Command::send(targets)))
});

let graph = StateGraph::new()
    .add_node("dispatch", dispatcher)
    .add_node("worker", worker_node)
    .add_deferred_node("collect", collector_node)
    .add_edge("worker", "collect")
    .add_edge("collect", END)
    .set_entry_point("dispatch")
    .compile()?;
```

> **Note:** Full parallel fan-out for `Command::send()` is not yet implemented. Targets are currently processed sequentially. The deferred node infrastructure is in place for when parallel execution is added.

## Linear Graphs

A deferred node in a linear chain compiles and executes normally. The deferred marker only becomes meaningful when multiple edges converge on the same node:

```rust,ignore
let graph = StateGraph::new()
    .add_node("step1", step1)
    .add_deferred_node("step2", step2)
    .add_edge("step1", "step2")
    .add_edge("step2", END)
    .set_entry_point("step1")
    .compile()?;

let result = graph.invoke(AggState::default()).await?.into_state();
// Runs identically to a non-deferred node in a linear chain
```

## Notes

- **Deferred is a marker.** The current execution engine does not block on incoming edge completion -- it runs nodes in edge/command order. The marker is forward-looking infrastructure for future parallel fan-out support.
- **`is_deferred()` and `incoming_edge_count()` are introspection-only.** They let you validate graph topology in tests without affecting execution.
