use std::collections::HashMap;

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

fn build_linear_graph() -> synaptic_graph::CompiledGraph<MessageState> {
    StateGraph::new()
        .add_node("a", PassthroughNode)
        .add_node("b", PassthroughNode)
        .set_entry_point("a")
        .add_edge("a", "b")
        .add_edge("b", END)
        .compile()
        .unwrap()
}

fn build_conditional_graph_with_path_map() -> synaptic_graph::CompiledGraph<MessageState> {
    StateGraph::new()
        .add_node("agent", PassthroughNode)
        .add_node("tools", PassthroughNode)
        .set_entry_point("agent")
        .add_conditional_edges_with_path_map(
            "agent",
            |_state: &MessageState| END.to_string(),
            HashMap::from([
                ("tools".to_string(), "tools".to_string()),
                (END.to_string(), END.to_string()),
            ]),
        )
        .add_edge("tools", "agent")
        .compile()
        .unwrap()
}

fn build_conditional_graph_without_path_map() -> synaptic_graph::CompiledGraph<MessageState> {
    StateGraph::new()
        .add_node("agent", PassthroughNode)
        .add_node("tools", PassthroughNode)
        .set_entry_point("agent")
        .add_conditional_edges("agent", |_state: &MessageState| END.to_string())
        .add_edge("tools", "agent")
        .compile()
        .unwrap()
}

// === Mermaid tests ===

#[test]
fn mermaid_linear_graph() {
    let graph = build_linear_graph();
    let mermaid = graph.draw_mermaid();

    assert!(mermaid.starts_with("graph TD"));
    assert!(mermaid.contains("__start__([\"__start__\"])"));
    assert!(mermaid.contains("__end__([\"__end__\"])"));
    assert!(mermaid.contains("a[\"a\"]"));
    assert!(mermaid.contains("b[\"b\"]"));
    assert!(mermaid.contains("__start__ --> a"));
    assert!(mermaid.contains("a --> b"));
    assert!(mermaid.contains("b --> __end__"));
}

#[test]
fn mermaid_conditional_with_path_map() {
    let graph = build_conditional_graph_with_path_map();
    let mermaid = graph.draw_mermaid();

    assert!(mermaid.contains("agent -.-> |__end__| __end__"));
    assert!(mermaid.contains("agent -.-> |tools| tools"));
    assert!(mermaid.contains("tools --> agent"));
    // Should NOT have the "path_map not provided" comment
    assert!(!mermaid.contains("path_map not provided"));
}

#[test]
fn mermaid_conditional_without_path_map() {
    let graph = build_conditional_graph_without_path_map();
    let mermaid = graph.draw_mermaid();

    assert!(mermaid.contains("path_map not provided"));
}

#[test]
fn mermaid_deterministic_output() {
    let graph = build_conditional_graph_with_path_map();
    let mermaid1 = graph.draw_mermaid();
    let mermaid2 = graph.draw_mermaid();
    assert_eq!(mermaid1, mermaid2);
}

// === ASCII tests ===

#[test]
fn ascii_simple_graph() {
    let graph = build_linear_graph();
    let ascii = graph.draw_ascii();

    assert!(ascii.starts_with("Graph:"));
    assert!(ascii.contains("Nodes: a, b"));
    assert!(ascii.contains("Entry: __start__ -> a"));
    assert!(ascii.contains("a -> b"));
    assert!(ascii.contains("b -> __end__"));
}

#[test]
fn ascii_conditional_with_path_map() {
    let graph = build_conditional_graph_with_path_map();
    let ascii = graph.draw_ascii();

    assert!(ascii.contains("[conditional]"));
    assert!(ascii.contains("agent ->"));
}

#[test]
fn ascii_conditional_without_path_map() {
    let graph = build_conditional_graph_without_path_map();
    let ascii = graph.draw_ascii();

    assert!(ascii.contains("agent -> ???  [conditional]"));
}

// === DOT tests ===

#[test]
fn dot_basic_format() {
    let graph = build_linear_graph();
    let dot = graph.draw_dot();

    assert!(dot.starts_with("digraph G {"));
    assert!(dot.ends_with('}'));
    assert!(dot.contains("\"__start__\" [shape=oval]"));
    assert!(dot.contains("\"a\" [shape=box]"));
    assert!(dot.contains("\"__start__\" -> \"a\" [style=solid]"));
    assert!(dot.contains("\"a\" -> \"b\" [style=solid]"));
    assert!(dot.contains("\"b\" -> \"__end__\" [style=solid]"));
}

#[test]
fn dot_conditional_dashed_edges() {
    let graph = build_conditional_graph_with_path_map();
    let dot = graph.draw_dot();

    assert!(dot.contains("\"agent\" -> \"tools\" [style=dashed, label=\"tools\"]"));
    assert!(dot.contains("\"agent\" -> \"__end__\" [style=dashed, label=\"__end__\"]"));
}

// === Builder validation tests ===

#[test]
fn invalid_path_map_target_rejected() {
    let result = StateGraph::new()
        .add_node("a", PassthroughNode)
        .set_entry_point("a")
        .add_conditional_edges_with_path_map(
            "a",
            |_state: &MessageState| END.to_string(),
            HashMap::from([("nonexistent".to_string(), "nonexistent".to_string())]),
        )
        .compile();

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("nonexistent"), "got: {msg}");
    assert!(msg.contains("not found"), "got: {msg}");
}

#[test]
fn end_target_in_path_map_accepted() {
    let result = StateGraph::new()
        .add_node("a", PassthroughNode)
        .set_entry_point("a")
        .add_conditional_edges_with_path_map(
            "a",
            |_state: &MessageState| END.to_string(),
            HashMap::from([(END.to_string(), END.to_string())]),
        )
        .compile();

    assert!(result.is_ok());
}

// === Display trait test ===

#[test]
fn display_matches_draw_ascii() {
    let graph = build_linear_graph();
    let display_output = format!("{graph}");
    let ascii_output = graph.draw_ascii();
    assert_eq!(display_output, ascii_output);
}

// === draw_png (graphviz) tests ===

#[test]
fn draw_png_produces_valid_file() {
    // Skip if `dot` is not installed
    if std::process::Command::new("dot")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("skipping draw_png test: graphviz 'dot' not found in PATH");
        return;
    }

    let graph = build_linear_graph();
    let dir = std::env::temp_dir().join("synapse_test_draw_png");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_graph.png");

    graph.draw_png(&path).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    // PNG files start with the magic bytes 0x89 P N G
    assert!(bytes.len() > 8, "PNG file too small");
    assert_eq!(&bytes[1..4], b"PNG", "not a valid PNG file");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn draw_png_conditional_graph() {
    if std::process::Command::new("dot")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("skipping draw_png test: graphviz 'dot' not found in PATH");
        return;
    }

    let graph = build_conditional_graph_with_path_map();
    let dir = std::env::temp_dir().join("synapse_test_draw_png_cond");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("conditional.png");

    graph.draw_png(&path).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[1..4], b"PNG");

    let _ = std::fs::remove_dir_all(&dir);
}

// === draw_mermaid_png / draw_mermaid_svg tests ===
// These require network access and are ignored by default.

#[tokio::test]
#[ignore]
async fn draw_mermaid_png_produces_valid_file() {
    let graph = build_linear_graph();
    let dir = std::env::temp_dir().join("synapse_test_mermaid_png");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("mermaid_graph.jpg");

    graph.draw_mermaid_png(&path).await.unwrap();

    let bytes = std::fs::read(&path).unwrap();
    // mermaid.ink /img/ endpoint returns JPEG (magic bytes: FF D8 FF)
    assert!(bytes.len() > 4, "image file too small");
    assert_eq!(bytes[0], 0xFF, "not a valid JPEG file");
    assert_eq!(bytes[1], 0xD8, "not a valid JPEG file");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[ignore]
async fn draw_mermaid_svg_produces_valid_file() {
    let graph = build_linear_graph();
    let dir = std::env::temp_dir().join("synapse_test_mermaid_svg");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("mermaid_graph.svg");

    graph.draw_mermaid_svg(&path).await.unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<svg"), "not a valid SVG file");

    let _ = std::fs::remove_dir_all(&dir);
}
