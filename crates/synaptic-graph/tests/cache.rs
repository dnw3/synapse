use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{CachePolicy, Node, NodeOutput, State, StateGraph, END};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CounterState {
    counter: usize,
    visited: Vec<String>,
}

impl State for CounterState {
    fn merge(&mut self, other: Self) {
        self.counter += other.counter;
        self.visited.extend(other.visited);
    }
}

/// A node that tracks how many times it's been executed.
struct TrackedNode {
    name: String,
    call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Node<CounterState> for TrackedNode {
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state.into())
    }
}

#[tokio::test]
async fn cached_node_executes_once_for_same_input() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let graph = StateGraph::new()
        .add_node_with_cache(
            "a",
            TrackedNode {
                name: "a".into(),
                call_count: call_count.clone(),
            },
            CachePolicy::new(Duration::from_secs(60)),
        )
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    // First invocation
    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(result.counter, 1);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second invocation with same input — should use cache
    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(result.counter, 1);
    assert_eq!(call_count.load(Ordering::SeqCst), 1); // still 1 — cache hit
}

#[tokio::test]
async fn cached_node_re_executes_for_different_input() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let graph = StateGraph::new()
        .add_node_with_cache(
            "a",
            TrackedNode {
                name: "a".into(),
                call_count: call_count.clone(),
            },
            CachePolicy::new(Duration::from_secs(60)),
        )
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    // First invocation with counter=0
    let _ = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second invocation with counter=5 — different input, no cache hit
    let _ = graph
        .invoke(CounterState {
            counter: 5,
            visited: vec![],
        })
        .await
        .unwrap()
        .into_state();
    assert_eq!(call_count.load(Ordering::SeqCst), 2); // called again
}

#[tokio::test]
async fn cache_expires_after_ttl() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let graph = StateGraph::new()
        .add_node_with_cache(
            "a",
            TrackedNode {
                name: "a".into(),
                call_count: call_count.clone(),
            },
            CachePolicy::new(Duration::from_millis(50)),
        )
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    // First invocation
    let _ = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Second invocation — cache expired, re-executes
    let _ = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn uncached_node_always_executes() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let graph = StateGraph::new()
        .add_node(
            "a",
            TrackedNode {
                name: "a".into(),
                call_count: call_count.clone(),
            },
        )
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let _ = graph.invoke(CounterState::default()).await.unwrap();
    let _ = graph.invoke(CounterState::default()).await.unwrap();
    let _ = graph.invoke(CounterState::default()).await.unwrap();

    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}
