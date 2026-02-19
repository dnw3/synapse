use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_graph::{MessageState, Node, NodeOutput, StateGraph, END};

/// A simple passthrough node for testing.
struct PassthroughNode;

#[async_trait]
impl Node<MessageState> for PassthroughNode {
    async fn process(
        &self,
        state: MessageState,
    ) -> Result<NodeOutput<MessageState>, SynapticError> {
        Ok(state.into())
    }
}

#[test]
fn build_simple_graph() {
    let result = StateGraph::new()
        .add_node("a", PassthroughNode)
        .add_node("b", PassthroughNode)
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile();

    assert!(result.is_ok());
}

#[test]
fn missing_entry_point_fails() {
    let result = StateGraph::<MessageState>::new()
        .add_node("a", PassthroughNode)
        .compile();

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("no entry point"), "got: {msg}");
}

#[test]
fn missing_node_in_edge_fails() {
    let result = StateGraph::<MessageState>::new()
        .add_node("a", PassthroughNode)
        .set_entry_point("a")
        .add_edge("a", "nonexistent")
        .compile();

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("not found"), "got: {msg}");
}

#[test]
fn conditional_edges_build() {
    let result = StateGraph::new()
        .add_node("a", PassthroughNode)
        .add_node("b", PassthroughNode)
        .set_entry_point("a")
        .add_conditional_edges("a", |_state: &MessageState| "b".to_string())
        .add_edge("b", END)
        .compile();

    assert!(result.is_ok());
}
