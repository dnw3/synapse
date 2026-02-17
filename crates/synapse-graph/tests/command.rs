use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synapse_core::SynapseError;
use synapse_graph::{GraphCommand, GraphContext, Node, State, StateGraph, StreamMode, END};
use tokio::sync::Mutex;

/// Test state with a counter and log of visited nodes.
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

/// A node that can optionally issue a command via a shared GraphContext.
struct ContextAwareNode {
    name: String,
    shared_ctx: Arc<Mutex<Option<GraphContext>>>,
    command: Option<GraphCommand>,
}

#[async_trait]
impl Node<CounterState> for ContextAwareNode {
    async fn process(&self, mut state: CounterState) -> Result<CounterState, SynapseError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        if let Some(ref cmd) = self.command {
            let ctx_guard = self.shared_ctx.lock().await;
            if let Some(ref ctx) = *ctx_guard {
                match cmd {
                    GraphCommand::Goto(target) => ctx.goto(target).await,
                    GraphCommand::End => ctx.end().await,
                }
            }
        }
        Ok(state)
    }
}

// ---------------------------------------------------------------------------
// GraphContext unit tests (public API only)
// ---------------------------------------------------------------------------

#[test]
fn graph_context_default() {
    let ctx = GraphContext::default();
    let _ = format!("{:?}", ctx); // Debug works
}

#[test]
fn graph_context_clone_shares_state() {
    // Clone uses Arc internally, so clones share the same underlying state.
    let ctx1 = GraphContext::new();
    let ctx2 = ctx1.clone();
    // Both are GraphContext instances sharing the same Arc.
    let _ = (ctx1, ctx2);
}

#[test]
fn graph_command_debug() {
    let goto = GraphCommand::Goto("target".to_string());
    let end = GraphCommand::End;
    assert!(format!("{:?}", goto).contains("target"));
    assert!(format!("{:?}", end).contains("End"));
}

#[test]
fn graph_command_clone() {
    let cmd = GraphCommand::Goto("node_a".to_string());
    let cloned = cmd.clone();
    match cloned {
        GraphCommand::Goto(t) => assert_eq!(t, "node_a"),
        _ => panic!("expected Goto"),
    }
}

// ---------------------------------------------------------------------------
// Integration: Goto command overrides routing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn goto_command_skips_node() {
    let shared_ctx: Arc<Mutex<Option<GraphContext>>> = Arc::new(Mutex::new(None));

    let graph = StateGraph::new()
        .add_node(
            "a",
            ContextAwareNode {
                name: "a".into(),
                shared_ctx: shared_ctx.clone(),
                command: Some(GraphCommand::Goto("c".to_string())), // skip b
            },
        )
        .add_node(
            "b",
            ContextAwareNode {
                name: "b".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_node(
            "c",
            ContextAwareNode {
                name: "c".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    // Inject the graph's context into the shared slot
    {
        let mut guard = shared_ctx.lock().await;
        *guard = Some(graph.context().clone());
    }

    let result = graph.invoke(CounterState::default()).await.unwrap();

    // "a" executes, issues Goto("c"), "b" is skipped, "c" executes
    assert_eq!(result.visited, vec!["a", "c"]);
    assert_eq!(result.counter, 2);
}

#[tokio::test]
async fn goto_command_redirects_to_earlier_node() {
    // Test that Goto can redirect to a node that would normally come earlier,
    // creating a loop (but we rely on the counter to eventually end).
    let shared_ctx: Arc<Mutex<Option<GraphContext>>> = Arc::new(Mutex::new(None));

    /// A node that issues Goto back to "a" until counter reaches threshold, then goes to END.
    struct LoopNode {
        shared_ctx: Arc<Mutex<Option<GraphContext>>>,
        threshold: usize,
    }

    #[async_trait]
    impl Node<CounterState> for LoopNode {
        async fn process(&self, mut state: CounterState) -> Result<CounterState, SynapseError> {
            state.counter += 1;
            state.visited.push("loop".to_string());
            let ctx_guard = self.shared_ctx.lock().await;
            if let Some(ref ctx) = *ctx_guard {
                if state.counter < self.threshold {
                    ctx.goto("a").await;
                } else {
                    ctx.end().await;
                }
            }
            Ok(state)
        }
    }

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node(
            "loop",
            LoopNode {
                shared_ctx: shared_ctx.clone(),
                threshold: 4, // run until counter >= 4
            },
        )
        .add_edge("a", "loop")
        .add_edge("loop", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    {
        let mut guard = shared_ctx.lock().await;
        *guard = Some(graph.context().clone());
    }

    let result = graph.invoke(CounterState::default()).await.unwrap();

    // a(1) -> loop(2, goto a) -> a(3) -> loop(4, end)
    assert_eq!(result.counter, 4);
    assert_eq!(result.visited, vec!["a", "loop", "a", "loop"]);
}

// ---------------------------------------------------------------------------
// Integration: End command stops execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn end_command_stops_execution() {
    let shared_ctx: Arc<Mutex<Option<GraphContext>>> = Arc::new(Mutex::new(None));

    let graph = StateGraph::new()
        .add_node(
            "a",
            ContextAwareNode {
                name: "a".into(),
                shared_ctx: shared_ctx.clone(),
                command: Some(GraphCommand::End), // end immediately after a
            },
        )
        .add_node(
            "b",
            ContextAwareNode {
                name: "b".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    {
        let mut guard = shared_ctx.lock().await;
        *guard = Some(graph.context().clone());
    }

    let result = graph.invoke(CounterState::default()).await.unwrap();

    // "a" executes, End command stops execution, "b" never runs
    assert_eq!(result.visited, vec!["a"]);
    assert_eq!(result.counter, 1);
}

// ---------------------------------------------------------------------------
// Integration: No command preserves normal routing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_command_preserves_normal_routing() {
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

    let result = graph.invoke(CounterState::default()).await.unwrap();
    assert_eq!(result.visited, vec!["a", "b", "c"]);
    assert_eq!(result.counter, 3);
}

// ---------------------------------------------------------------------------
// Integration: Command in streaming mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn goto_command_works_in_stream_mode() {
    let shared_ctx: Arc<Mutex<Option<GraphContext>>> = Arc::new(Mutex::new(None));

    let graph = StateGraph::new()
        .add_node(
            "a",
            ContextAwareNode {
                name: "a".into(),
                shared_ctx: shared_ctx.clone(),
                command: Some(GraphCommand::Goto("c".to_string())),
            },
        )
        .add_node(
            "b",
            ContextAwareNode {
                name: "b".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_node(
            "c",
            ContextAwareNode {
                name: "c".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    {
        let mut guard = shared_ctx.lock().await;
        *guard = Some(graph.context().clone());
    }

    let events: Vec<_> = graph
        .stream(CounterState::default(), StreamMode::Values)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Should have 2 events: "a" then "c" (skipped "b")
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].node, "a");
    assert_eq!(events[1].node, "c");
    assert_eq!(events[1].state.visited, vec!["a", "c"]);
}

#[tokio::test]
async fn end_command_works_in_stream_mode() {
    let shared_ctx: Arc<Mutex<Option<GraphContext>>> = Arc::new(Mutex::new(None));

    let graph = StateGraph::new()
        .add_node(
            "a",
            ContextAwareNode {
                name: "a".into(),
                shared_ctx: shared_ctx.clone(),
                command: Some(GraphCommand::End),
            },
        )
        .add_node(
            "b",
            ContextAwareNode {
                name: "b".into(),
                shared_ctx: shared_ctx.clone(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    {
        let mut guard = shared_ctx.lock().await;
        *guard = Some(graph.context().clone());
    }

    let events: Vec<_> = graph
        .stream(CounterState::default(), StreamMode::Values)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Should have only 1 event: "a" then end
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node, "a");
    assert_eq!(events[0].state.counter, 1);
}
