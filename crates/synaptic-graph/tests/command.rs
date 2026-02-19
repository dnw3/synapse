use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;
use synaptic_graph::{Command, Node, NodeOutput, State, StateGraph, StreamMode, END};

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
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        Ok(state.into())
    }
}

/// A node that increments counter, records its name, and optionally issues a Command.
/// When issuing a command, it packages the state delta (counter=1, visited=[name])
/// into a `goto_with_update` or `end` command with an update.
struct CommandNode {
    name: String,
    command: Option<CommandKind>,
}

/// The kind of command a CommandNode can issue.
#[derive(Clone)]
enum CommandKind {
    Goto(String),
    End,
}

#[async_trait]
impl Node<CounterState> for CommandNode {
    async fn process(
        &self,
        mut state: CounterState,
    ) -> Result<NodeOutput<CounterState>, SynapticError> {
        state.counter += 1;
        state.visited.push(self.name.clone());
        match &self.command {
            Some(CommandKind::Goto(target)) => {
                // Return a goto command; pass the full updated state as NodeOutput::State
                // won't work since we need routing. Use goto_with_update with the delta.
                let delta = CounterState {
                    counter: 1,
                    visited: vec![self.name.clone()],
                };
                Ok(NodeOutput::Command(Command::goto_with_update(
                    target.clone(),
                    delta,
                )))
            }
            Some(CommandKind::End) => {
                let delta = CounterState {
                    counter: 1,
                    visited: vec![self.name.clone()],
                };
                Ok(NodeOutput::Command(Command::goto_with_update(END, delta)))
            }
            None => Ok(state.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Command unit tests
// ---------------------------------------------------------------------------

#[test]
fn command_debug() {
    let goto: Command<CounterState> = Command::goto("target");
    let end: Command<CounterState> = Command::end();
    assert!(format!("{:?}", goto).contains("goto"));
    assert!(format!("{:?}", end).contains("goto")); // end uses goto to __end__
}

#[test]
fn command_clone() {
    let cmd: Command<CounterState> = Command::goto("node_a");
    let cloned = cmd.clone();
    let dbg = format!("{:?}", cloned);
    assert!(dbg.contains("goto"));
}

// ---------------------------------------------------------------------------
// Integration: Goto command overrides routing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn goto_command_skips_node() {
    let graph = StateGraph::new()
        .add_node(
            "a",
            CommandNode {
                name: "a".into(),
                command: Some(CommandKind::Goto("c".to_string())), // skip b
            },
        )
        .add_node(
            "b",
            CommandNode {
                name: "b".into(),
                command: None,
            },
        )
        .add_node(
            "c",
            CommandNode {
                name: "c".into(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", "c")
        .add_edge("c", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();

    // "a" executes, issues Goto("c"), "b" is skipped, "c" executes
    assert_eq!(result.visited, vec!["a", "c"]);
    assert_eq!(result.counter, 2);
}

#[tokio::test]
async fn goto_command_redirects_to_earlier_node() {
    // Test that Goto can redirect to a node that would normally come earlier,
    // creating a loop (but we rely on the counter to eventually end).

    /// A node that issues Goto back to "a" until counter reaches threshold, then goes to END.
    struct LoopNode {
        threshold: usize,
    }

    #[async_trait]
    impl Node<CounterState> for LoopNode {
        async fn process(
            &self,
            mut state: CounterState,
        ) -> Result<NodeOutput<CounterState>, SynapticError> {
            state.counter += 1;
            state.visited.push("loop".to_string());
            let delta = CounterState {
                counter: 1,
                visited: vec!["loop".to_string()],
            };
            if state.counter < self.threshold {
                Ok(NodeOutput::Command(Command::goto_with_update("a", delta)))
            } else {
                Ok(NodeOutput::Command(Command::goto_with_update(END, delta)))
            }
        }
    }

    let graph = StateGraph::new()
        .add_node("a", IncrementNode { name: "a".into() })
        .add_node(
            "loop",
            LoopNode {
                threshold: 4, // run until counter >= 4
            },
        )
        .add_edge("a", "loop")
        .add_edge("loop", END)
        .set_entry_point("a")
        .compile()
        .unwrap();

    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();

    // a(1) -> loop(2, goto a) -> a(3) -> loop(4, end)
    assert_eq!(result.counter, 4);
    assert_eq!(result.visited, vec!["a", "loop", "a", "loop"]);
}

// ---------------------------------------------------------------------------
// Integration: End command stops execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn end_command_stops_execution() {
    let graph = StateGraph::new()
        .add_node(
            "a",
            CommandNode {
                name: "a".into(),
                command: Some(CommandKind::End), // end immediately after a
            },
        )
        .add_node(
            "b",
            CommandNode {
                name: "b".into(),
                command: None,
            },
        )
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

    let result = graph
        .invoke(CounterState::default())
        .await
        .unwrap()
        .into_state();
    assert_eq!(result.visited, vec!["a", "b", "c"]);
    assert_eq!(result.counter, 3);
}

// ---------------------------------------------------------------------------
// Integration: Command in streaming mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn goto_command_works_in_stream_mode() {
    let graph = StateGraph::new()
        .add_node(
            "a",
            CommandNode {
                name: "a".into(),
                command: Some(CommandKind::Goto("c".to_string())),
            },
        )
        .add_node(
            "b",
            CommandNode {
                name: "b".into(),
                command: None,
            },
        )
        .add_node(
            "c",
            CommandNode {
                name: "c".into(),
                command: None,
            },
        )
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

    // Should have 2 events: "a" then "c" (skipped "b")
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].node, "a");
    assert_eq!(events[1].node, "c");
    assert_eq!(events[1].state.visited, vec!["a", "c"]);
}

#[tokio::test]
async fn end_command_works_in_stream_mode() {
    let graph = StateGraph::new()
        .add_node(
            "a",
            CommandNode {
                name: "a".into(),
                command: Some(CommandKind::End),
            },
        )
        .add_node(
            "b",
            CommandNode {
                name: "b".into(),
                command: None,
            },
        )
        .add_edge("a", "b")
        .add_edge("b", END)
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

    // Should have only 1 event: "a" then end
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node, "a");
    assert_eq!(events[0].state.counter, 1);
}
