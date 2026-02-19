use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{Node, NodeOutput, State, StateGraph, END};

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
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state.into())
    }
}

#[tokio::test]
async fn deferred_node_compiles_and_runs() {
    // Deferred node in a simple linear graph — should behave normally
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_deferred_node("b", IncrementNode { name: "b".into() })
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

    assert_eq!(result.counter, 2);
    assert_eq!(result.visited, vec!["a", "b"]);
}

#[tokio::test]
async fn deferred_node_is_queryable() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_deferred_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    assert!(graph.is_deferred("b"));
    assert!(!graph.is_deferred("a"));
}

#[tokio::test]
async fn incoming_edge_count_computed_correctly() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_deferred_node("c", IncrementNode { name: "c".into() })
        .add_edge("a", "c")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    assert_eq!(graph.incoming_edge_count("c"), 2);
    assert_eq!(graph.incoming_edge_count("a"), 0);
    assert_eq!(graph.incoming_edge_count("b"), 0);
}
