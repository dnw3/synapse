use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{CheckpointConfig, MemorySaver, Node, State, StateGraph, END};

/// Test state with a counter and visited log.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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

/// Node that increments counter and records its name.
struct IncrementNode {
    name: String,
}

#[async_trait]
impl Node<CounterState> for IncrementNode {
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<synaptic_graph::NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state.into())
    }
}

#[tokio::test]
async fn get_state_returns_none_for_empty_thread() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("nonexistent-thread");
    let state: Option<CounterState> = graph.get_state(&config).await.unwrap();
    assert!(state.is_none());
}

#[tokio::test]
async fn get_state_returns_latest_state() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("thread-1");
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await
        .unwrap();

    let state: CounterState = graph.get_state(&config).await.unwrap().unwrap();
    assert_eq!(state.counter, 2);
    assert_eq!(state.visited, vec!["a", "b"]);
}

#[tokio::test]
async fn get_state_after_interrupt_before() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .interrupt_before(vec!["b".to_string()])
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("thread-2");
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    // State should reflect execution of "a" only
    let state: CounterState = graph.get_state(&config).await.unwrap().unwrap();
    assert_eq!(state.counter, 1);
    assert_eq!(state.visited, vec!["a"]);
}

#[tokio::test]
async fn get_state_errors_without_checkpointer() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let config = CheckpointConfig::new("thread-x");
    let result: Result<Option<CounterState>, _> = graph.get_state(&config).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("no checkpointer configured"));
}

#[tokio::test]
async fn get_state_history_returns_empty_for_new_thread() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("nonexistent");
    let history: Vec<(CounterState, Option<String>)> =
        graph.get_state_history(&config).await.unwrap();
    assert!(history.is_empty());
}

#[tokio::test]
async fn get_state_history_returns_all_checkpoints() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("thread-hist");
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await
        .unwrap();

    let history: Vec<(CounterState, Option<String>)> =
        graph.get_state_history(&config).await.unwrap();

    // Two nodes executed -> two checkpoints saved (one after each node)
    assert_eq!(history.len(), 2);

    // First checkpoint: after "a", next_node should be "b"
    assert_eq!(history[0].0.counter, 1);
    assert_eq!(history[0].0.visited, vec!["a"]);
    assert_eq!(history[0].1.as_deref(), Some("b"));

    // Second checkpoint: after "b", next_node should be END
    assert_eq!(history[1].0.counter, 2);
    assert_eq!(history[1].0.visited, vec!["a", "b"]);
    assert_eq!(history[1].1.as_deref(), Some("__end__"));
}

#[tokio::test]
async fn get_state_history_errors_without_checkpointer() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let config = CheckpointConfig::new("thread-y");
    let result: Result<Vec<(CounterState, Option<String>)>, _> =
        graph.get_state_history(&config).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("no checkpointer configured"));
}

#[tokio::test]
async fn get_state_history_with_interrupt_shows_partial() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_node("c", IncrementNode { name: "c".into() })
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .interrupt_before(vec!["c".to_string()])
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("thread-partial");
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    let history: Vec<(CounterState, Option<String>)> =
        graph.get_state_history(&config).await.unwrap();

    // "a" runs (checkpoint saved), "b" runs (checkpoint saved), then interrupt before "c" (another checkpoint)
    assert_eq!(history.len(), 3);

    // First: after "a", next = "b"
    assert_eq!(history[0].0.counter, 1);
    assert_eq!(history[0].1.as_deref(), Some("b"));

    // Second: after "b", next = "c"
    assert_eq!(history[1].0.counter, 2);
    assert_eq!(history[1].1.as_deref(), Some("c"));

    // Third: interrupt checkpoint before "c", next = "c"
    assert_eq!(history[2].0.counter, 2);
    assert_eq!(history[2].1.as_deref(), Some("c"));
}
