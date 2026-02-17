use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapseError;
use synaptic_graph::{CheckpointConfig, Checkpointer, MemorySaver, Node, State, StateGraph, END};

/// A simple test state with a counter and a log of visited nodes.
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

/// Node that increments counter and records its name.
struct IncrementNode {
    name: String,
}

#[async_trait]
impl Node<CounterState> for IncrementNode {
    async fn process(&self, mut state: CounterState) -> Result<CounterState, SynapseError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state)
    }
}

#[tokio::test]
async fn simple_linear_graph() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let result = graph.invoke(CounterState::default()).await.unwrap();
    assert_eq!(result.counter, 2);
    assert_eq!(result.visited, vec!["a", "b"]);
}

#[tokio::test]
async fn conditional_routing() {
    // Route to "left" if counter < 1, else route to "right"
    let graph = StateGraph::new()
        .add_node(
            "start",
            IncrementNode {
                name: "start".into(),
            },
        )
        .add_node(
            "left",
            IncrementNode {
                name: "left".into(),
            },
        )
        .add_node(
            "right",
            IncrementNode {
                name: "right".into(),
            },
        )
        .set_entry_point("start")
        .add_conditional_edges("start", |state: &CounterState| {
            if state.counter < 2 {
                "left".to_string()
            } else {
                "right".to_string()
            }
        })
        .add_edge("left", END)
        .add_edge("right", END)
        .compile()
        .unwrap();

    // counter starts at 0, start increments to 1 => route to "left"
    let result = graph.invoke(CounterState::default()).await.unwrap();
    assert_eq!(result.visited, vec!["start", "left"]);

    // counter starts at 5, start increments to 6 => route to "right"
    let state = CounterState {
        counter: 5,
        visited: vec![],
    };
    let result = graph.invoke(state).await.unwrap();
    assert_eq!(result.visited, vec!["start", "right"]);
}

#[tokio::test]
async fn interrupt_before_stops_execution() {
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
        .with_checkpointer(saver.clone());

    let config = CheckpointConfig::new("thread-1");
    let result = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    // Should fail with interrupt error
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("interrupted before node 'b'"), "got: {msg}");

    // Checkpoint should have been saved
    let cp = saver.get(&config).await.unwrap().unwrap();
    assert!(cp.next_node.as_deref() == Some("b"));
}

#[tokio::test]
async fn interrupt_after_stops_execution() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .interrupt_after(vec!["a".to_string()])
        .compile()
        .unwrap()
        .with_checkpointer(saver.clone());

    let config = CheckpointConfig::new("thread-2");
    let result = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("interrupted after node 'a'"), "got: {msg}");
}

#[tokio::test]
async fn resume_from_checkpoint() {
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
        .with_checkpointer(saver.clone());

    let config = CheckpointConfig::new("thread-3");

    // First invocation — interrupted before "b"
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    // Now remove the interrupt and re-compile to allow resumption.
    // But since CompiledGraph already has the interrupt, let's just build a new graph
    // that does NOT interrupt and share the same checkpointer.
    let graph2 = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver.clone());

    // Resume — should pick up from checkpoint and run "b"
    let result = graph2
        .invoke_with_config(CounterState::default(), Some(config))
        .await
        .unwrap();

    // The checkpoint state had counter=1, visited=["a"], then "b" runs
    assert_eq!(result.counter, 2);
    assert_eq!(result.visited, vec!["a", "b"]);
}

#[tokio::test]
async fn update_state_modifies_checkpoint() {
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
        .with_checkpointer(saver.clone());

    let config = CheckpointConfig::new("thread-4");

    // Run until interrupt
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    // Update state: add 10 to counter
    let update = CounterState {
        counter: 10,
        visited: vec!["injected".to_string()],
    };
    graph.update_state(&config, update).await.unwrap();

    // Check the checkpoint was updated
    let cp = saver.get(&config).await.unwrap().unwrap();
    let state: CounterState = serde_json::from_value(cp.state).unwrap();
    // Original: counter=1, visited=["a"]; merged with counter=10, visited=["injected"]
    assert_eq!(state.counter, 11);
    assert!(state.visited.contains(&"a".to_string()));
    assert!(state.visited.contains(&"injected".to_string()));
}

#[tokio::test]
async fn max_iterations_guard() {
    // Create a cycle: a -> b -> a (no exit)
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", "a")
        .set_entry_point("a")
        .compile()
        .unwrap();

    let err = graph.invoke(CounterState::default()).await.unwrap_err();
    // Should error after 100 iterations (safety guard)
    assert!(err.to_string().contains("max iterations"), "got: {err}");
}
