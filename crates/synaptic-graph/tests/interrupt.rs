use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use synaptic_core::SynapticError;
use synaptic_graph::{
    interrupt, CheckpointConfig, Command, MemorySaver, Node, NodeOutput, State, StateGraph, END,
};

/// Test state with a counter and visited log.
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

/// A node that increments counter and records its name.
struct IncrementNode {
    name: String,
}

#[async_trait]
impl Node<CounterState> for IncrementNode {
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state.into())
    }
}

/// A node that interrupts execution with a value.
struct InterruptNode {
    name: String,
    interrupt_value: serde_json::Value,
}

#[async_trait]
impl Node<CounterState> for InterruptNode {
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        // Use the interrupt() function to pause execution
        Ok(interrupt(self.interrupt_value.clone()))
    }
}

// ---------------------------------------------------------------------------
// interrupt() function tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn interrupt_pauses_graph() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node(
            "b",
            InterruptNode {
                name: "b".into(),
                interrupt_value: json!({"question": "Continue?"}),
            },
        )
        .add_node("c", IncrementNode { name: "c".into() })
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("interrupt-thread");
    let result = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await
        .unwrap();

    // Graph should be interrupted
    assert!(result.is_interrupted());
    assert!(!result.is_complete());

    // The interrupt value should be the one we passed
    let iv = result.interrupt_value().unwrap();
    assert_eq!(iv["question"], "Continue?");

    // State should reflect execution up to the interrupt point
    // Note: interrupt() in the node means the node's state update is NOT applied
    // (it returns interrupt command, not a state update)
    let state = result.into_state();
    // "a" ran (counter=1), "b" issued interrupt (state not merged from b)
    assert_eq!(state.counter, 1);
    assert_eq!(state.visited, vec!["a"]);
}

#[tokio::test]
async fn interrupt_requires_checkpointer_for_resume() {
    let saver = Arc::new(MemorySaver::new());

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node(
            "b",
            InterruptNode {
                name: "b".into(),
                interrupt_value: json!("pause"),
            },
        )
        .add_node("c", IncrementNode { name: "c".into() })
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver.clone());

    let config = CheckpointConfig::new("resume-thread");

    // First invocation: should interrupt at "b"
    let result = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await
        .unwrap();
    assert!(result.is_interrupted());

    // Checkpoint should have been saved
    let saved: Option<CounterState> = graph.get_state(&config).await.unwrap();
    assert!(saved.is_some());
}

// ---------------------------------------------------------------------------
// GraphResult API tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn graph_result_complete_api() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let result = graph.invoke(CounterState::default()).await.unwrap();

    assert!(result.is_complete());
    assert!(!result.is_interrupted());
    assert!(result.interrupt_value().is_none());
    assert_eq!(result.state().counter, 1);
    assert_eq!(result.into_state().visited, vec!["a"]);
}

#[tokio::test]
async fn graph_result_interrupted_api() {
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
        .with_checkpointer(saver);

    let config = CheckpointConfig::new("api-test");
    let result = graph
        .invoke_with_config(CounterState::default(), Some(config))
        .await
        .unwrap();

    assert!(result.is_interrupted());
    assert!(!result.is_complete());
    assert!(result.interrupt_value().is_some());
    assert_eq!(result.state().counter, 1);

    // into_state() should still work on interrupted results
    let state = result.into_state();
    assert_eq!(state.visited, vec!["a"]);
}

// ---------------------------------------------------------------------------
// Command::update tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn command_update_only() {
    /// Node that returns Command::update with no routing override.
    struct UpdateNode;

    #[async_trait]
    impl Node<CounterState> for UpdateNode {
        async fn process(
            &self,
            _state: CounterState,
        ) -> Result<NodeOutput<CounterState>, SynapticError> {
            let delta = CounterState {
                counter: 10,
                visited: vec!["update".to_string()],
            };
            Ok(NodeOutput::Command(Command::update(delta)))
        }
    }

    let graph = StateGraph::new()
        .add_node("a", UpdateNode)
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();

    // "a" does Command::update (counter=10), then normal routing to "b" (counter+=1)
    assert_eq!(result.counter, 11);
    assert_eq!(result.visited, vec!["update", "b"]);
}
