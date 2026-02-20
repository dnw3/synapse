use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, ChatStream, SynapticError, Tool};
use synaptic::graph::{MessageState, Node, NodeOutput, StateGraph, END};
use synaptic::macros::tool;

/// Search the web
#[tool(name = "search")]
async fn dummy_search() -> Result<String, SynapticError> {
    Ok("result".to_string())
}

/// A dummy ChatModel for demonstration.
struct DummyModel;

#[async_trait]
impl ChatModel for DummyModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        unimplemented!("demo only")
    }
    fn stream_chat(&self, _request: ChatRequest) -> ChatStream<'_> {
        unimplemented!("demo only")
    }
}

/// A passthrough node for custom graph demonstration.
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

#[tokio::main]
async fn main() {
    println!("=== ReAct Agent Graph ===\n");

    let model: Arc<dyn ChatModel> = Arc::new(DummyModel);
    let tools: Vec<Arc<dyn Tool>> = vec![dummy_search()];
    let react_graph = synaptic::graph::create_react_agent(model, tools).unwrap();

    println!("--- Mermaid ---");
    println!("{}\n", react_graph.draw_mermaid());

    println!("--- ASCII ---");
    println!("{}\n", react_graph.draw_ascii());

    println!("--- DOT ---");
    println!("{}\n", react_graph.draw_dot());

    println!("--- Display ---");
    println!("{react_graph}\n");

    // draw_png via Graphviz (requires `dot` in PATH)
    match react_graph.draw_png("react_agent.png") {
        Ok(()) => println!("--- PNG (Graphviz) ---\nWritten to react_agent.png\n"),
        Err(e) => println!("--- PNG (Graphviz) ---\nSkipped: {e}\n"),
    }

    // draw_mermaid_png via mermaid.ink API (requires internet)
    match react_graph
        .draw_mermaid_png("react_agent_mermaid.png")
        .await
    {
        Ok(()) => println!("--- PNG (Mermaid.ink) ---\nWritten to react_agent_mermaid.png\n"),
        Err(e) => println!("--- PNG (Mermaid.ink) ---\nSkipped: {e}\n"),
    }

    println!("\n=== Custom Graph ===\n");

    let custom_graph = StateGraph::new()
        .add_node("fetch", PassthroughNode)
        .add_node("process", PassthroughNode)
        .add_node("summarize", PassthroughNode)
        .set_entry_point("fetch")
        .add_edge("fetch", "process")
        .add_conditional_edges_with_path_map(
            "process",
            |_state: &MessageState| "summarize".to_string(),
            HashMap::from([
                ("summarize".to_string(), "summarize".to_string()),
                ("fetch".to_string(), "fetch".to_string()),
            ]),
        )
        .add_edge("summarize", END)
        .compile()
        .unwrap();

    println!("--- Mermaid ---");
    println!("{}\n", custom_graph.draw_mermaid());

    println!("--- ASCII ---");
    println!("{}\n", custom_graph.draw_ascii());

    println!("--- DOT ---");
    println!("{}", custom_graph.draw_dot());
}
