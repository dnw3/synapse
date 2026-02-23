use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{
    CheckpointConfig, Node, State, StateGraph, StoreCheckpointer, StreamMode, END,
};

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
async fn stream_three_node_graph_values() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_node("c", IncrementNode { name: "c".into() })
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let events: Vec<_> = graph
        .stream(CounterState::default(), StreamMode::Values)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(events.len(), 3);

    assert_eq!(events[0].node, "a");
    assert_eq!(events[0].state.counter, 1);
    assert_eq!(events[0].state.visited, vec!["a"]);

    assert_eq!(events[1].node, "b");
    assert_eq!(events[1].state.counter, 2);
    assert_eq!(events[1].state.visited, vec!["a", "b"]);

    assert_eq!(events[2].node, "c");
    assert_eq!(events[2].state.counter, 3);
    assert_eq!(events[2].state.visited, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn stream_updates_mode() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let events: Vec<_> = graph
        .stream(CounterState::default(), StreamMode::Updates)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Both modes yield the same number of events (one per node)
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].node, "a");
    assert_eq!(events[1].node, "b");
}

#[tokio::test]
async fn stream_with_interrupt_after() {
    let saver = Arc::new(StoreCheckpointer::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )));
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

    let config = CheckpointConfig::new("stream-test-1");
    let events: Vec<_> = graph
        .stream_with_config(
            CounterState::default(),
            StreamMode::Values,
            Some(config.clone()),
        )
        .collect::<Vec<_>>()
        .await;

    // Should get one Ok event for "a", then one Err for the interrupt
    assert_eq!(events.len(), 2);
    assert!(events[0].is_ok());
    assert_eq!(events[0].as_ref().unwrap().node, "a");
    assert!(events[1].is_err());
    assert!(events[1]
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("interrupted after node 'a'"));
}

#[tokio::test]
async fn stream_with_checkpoint_resume() {
    let saver = Arc::new(StoreCheckpointer::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )));

    // First run with interrupt
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

    let config = CheckpointConfig::new("stream-test-2");
    let _ = graph
        .invoke_with_config(CounterState::default(), Some(config.clone()))
        .await;

    // Resume without interrupt
    let graph2 = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap()
        .with_checkpointer(saver.clone());

    let events: Vec<_> = graph2
        .stream_with_config(CounterState::default(), StreamMode::Values, Some(config))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Should resume at "b" and run just that node
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node, "b");
    assert_eq!(events[0].state.counter, 2);
    assert_eq!(events[0].state.visited, vec!["a", "b"]);
}

#[tokio::test]
async fn stream_conditional_routing() {
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

    let events: Vec<_> = graph
        .stream(CounterState::default(), StreamMode::Values)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].node, "start");
    assert_eq!(events[1].node, "left");
}
