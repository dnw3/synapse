use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{MultiGraphEvent, Node, NodeOutput, State, StateGraph, StreamMode, END};

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
async fn stream_modes_single_mode_values() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let events: Vec<MultiGraphEvent<CounterState>> = graph
        .stream_modes(CounterState::default(), vec![StreamMode::Values])
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].mode, StreamMode::Values);
    assert_eq!(events[0].event.node, "a");
    assert_eq!(events[0].event.state.counter, 1);
    assert_eq!(events[1].mode, StreamMode::Values);
    assert_eq!(events[1].event.node, "b");
    assert_eq!(events[1].event.state.counter, 2);
}

#[tokio::test]
async fn stream_modes_multiple_modes() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node("b", IncrementNode { name: "b".into() })
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let events: Vec<MultiGraphEvent<CounterState>> = graph
        .stream_modes(
            CounterState::default(),
            vec![StreamMode::Values, StreamMode::Updates],
        )
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // 2 nodes * 2 modes = 4 events
    assert_eq!(events.len(), 4);

    // Node "a" events
    assert_eq!(events[0].mode, StreamMode::Values);
    assert_eq!(events[0].event.node, "a");
    assert_eq!(events[0].event.state.counter, 1); // post-execution state

    assert_eq!(events[1].mode, StreamMode::Updates);
    assert_eq!(events[1].event.node, "a");
    assert_eq!(events[1].event.state.counter, 0); // pre-execution state (delta)

    // Node "b" events
    assert_eq!(events[2].mode, StreamMode::Values);
    assert_eq!(events[2].event.node, "b");
    assert_eq!(events[2].event.state.counter, 2);

    assert_eq!(events[3].mode, StreamMode::Updates);
    assert_eq!(events[3].event.node, "b");
    assert_eq!(events[3].event.state.counter, 1); // pre-execution state
}

#[tokio::test]
async fn stream_modes_empty_modes_no_events() {
    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_edge("a", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let events: Vec<_> = graph
        .stream_modes(CounterState::default(), vec![])
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // No modes requested = no events emitted (but graph still runs)
    assert_eq!(events.len(), 0);
}
