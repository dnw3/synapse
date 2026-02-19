# Node Caching

`CachePolicy` paired with `add_node_with_cache()` enables hash-based result caching on individual graph nodes. When the same serialized input state is seen within the TTL window, the cached output is returned without re-executing the node. Use this for expensive nodes (LLM calls, API requests) where identical inputs produce identical outputs.

## Setup

```rust,ignore
use std::time::Duration;
use synaptic_graph::{CachePolicy, FnNode, StateGraph, MessageState, END};
use synaptic_core::Message;

let expensive = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Expensive result"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node_with_cache(
        "llm_call",
        expensive,
        CachePolicy::new(Duration::from_secs(60)),
    )
    .add_edge("llm_call", END)
    .set_entry_point("llm_call")
    .compile()?;
```

## How It Works

1. Before executing a cached node, the graph serializes the current state to JSON and computes a hash.
2. If the cache contains a valid (non-expired) entry for that `(node_name, state_hash)`, the cached `NodeOutput` is returned immediately -- `process()` is not called.
3. On a cache miss, the node executes normally and the result is stored.

The cache is held in `Arc<RwLock<HashMap>>` inside `CompiledGraph`, persisting across multiple `invoke()` calls on the same instance.

## Example: Verifying Cache Hits

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use synaptic_core::SynapticError;
use synaptic_graph::{CachePolicy, Node, NodeOutput, State, StateGraph, END};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MyState { counter: usize }

impl State for MyState {
    fn merge(&mut self, other: Self) { self.counter += other.counter; }
}

struct TrackedNode { call_count: Arc<AtomicUsize> }

#[async_trait]
impl Node<MyState> for TrackedNode {
    async fn process(&self, mut state: MyState) -> Result<NodeOutput<MyState>, SynapticError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        state.counter += 1;
        Ok(state.into())
    }
}

let calls = Arc::new(AtomicUsize::new(0));
let graph = StateGraph::new()
    .add_node_with_cache("n", TrackedNode { call_count: calls.clone() },
        CachePolicy::new(Duration::from_secs(60)))
    .add_edge("n", END)
    .set_entry_point("n")
    .compile()?;

// First call: cache miss
graph.invoke(MyState::default()).await?;
assert_eq!(calls.load(Ordering::SeqCst), 1);

// Same input: cache hit -- node not called
graph.invoke(MyState::default()).await?;
assert_eq!(calls.load(Ordering::SeqCst), 1);

// Different input: cache miss
graph.invoke(MyState { counter: 5 }).await?;
assert_eq!(calls.load(Ordering::SeqCst), 2);
```

## TTL Expiry

Cached entries expire after the configured TTL. The next call with the same input re-executes the node:

```rust,ignore
let graph = StateGraph::new()
    .add_node_with_cache("n", my_node,
        CachePolicy::new(Duration::from_millis(100)))
    .add_edge("n", END)
    .set_entry_point("n")
    .compile()?;

graph.invoke(state.clone()).await?;                       // executes
tokio::time::sleep(Duration::from_millis(150)).await;
graph.invoke(state.clone()).await?;                       // executes again
```

## Mixing Cached and Uncached Nodes

Only nodes added with `add_node_with_cache()` are cached. Nodes added with `add_node()` always execute:

```rust,ignore
let graph = StateGraph::new()
    .add_node_with_cache("llm", llm_node, CachePolicy::new(Duration::from_secs(300)))
    .add_node("format", format_node) // always runs
    .set_entry_point("llm")
    .add_edge("llm", "format")
    .add_edge("format", END)
    .compile()?;
```

## Notes

- **State must implement `Serialize`.** The cache key is a hash of the JSON-serialized state.
- **Cache scope.** The cache lives on the `CompiledGraph` instance. A new `compile()` starts with an empty cache.
- **Works with Commands.** Cached entries store the full `NodeOutput`, including `Command` variants.
