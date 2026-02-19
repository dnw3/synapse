use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::send::Send;
use crate::State;

/// A command returned from a node to control graph flow.
///
/// Commands allow nodes to override normal edge-based routing,
/// update state, fan out to multiple nodes, or signal interrupts.
///
/// # Example
///
/// ```ignore
/// use synaptic_graph::{Command, NodeOutput, MessageState};
///
/// async fn my_node(state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
///     Ok(NodeOutput::Command(Command::goto("summary")))
/// }
/// ```
pub struct Command<S: State> {
    /// State update to merge before routing.
    pub(crate) update: Option<S>,
    /// Routing override.
    pub(crate) goto: Option<CommandGoto>,
    /// Interrupt: pause graph and return this value to the caller.
    pub(crate) interrupt_value: Option<Value>,
    /// Resume value passed from the caller to continue from interrupt.
    pub(crate) resume_value: Option<Value>,
}

impl<S: State> std::fmt::Debug for Command<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("has_update", &self.update.is_some())
            .field("goto", &self.goto)
            .field("interrupt_value", &self.interrupt_value)
            .field("resume_value", &self.resume_value)
            .finish()
    }
}

impl<S: State> Clone for Command<S> {
    fn clone(&self) -> Self {
        Self {
            update: self.update.clone(),
            goto: self.goto.clone(),
            interrupt_value: self.interrupt_value.clone(),
            resume_value: self.resume_value.clone(),
        }
    }
}

impl<S: State> Command<S> {
    /// Create a command that routes to a specific node.
    pub fn goto(node: impl Into<String>) -> Self {
        Self {
            update: None,
            goto: Some(CommandGoto::One(node.into())),
            interrupt_value: None,
            resume_value: None,
        }
    }

    /// Create a command that routes to a specific node with a state update.
    pub fn goto_with_update(node: impl Into<String>, update: S) -> Self {
        Self {
            update: Some(update),
            goto: Some(CommandGoto::One(node.into())),
            interrupt_value: None,
            resume_value: None,
        }
    }

    /// Create a command that fans out to multiple nodes (map-reduce).
    pub fn send(targets: Vec<Send>) -> Self {
        Self {
            update: None,
            goto: Some(CommandGoto::Many(targets)),
            interrupt_value: None,
            resume_value: None,
        }
    }

    /// Create a command that only updates state (no routing override).
    pub fn update(state: S) -> Self {
        Self {
            update: Some(state),
            goto: None,
            interrupt_value: None,
            resume_value: None,
        }
    }

    /// Create a resume command for continuing from an interrupt.
    ///
    /// Pass this as input to `graph.invoke()` to resume a previously
    /// interrupted graph execution.
    pub fn resume(value: Value) -> Self {
        Self {
            update: None,
            goto: None,
            interrupt_value: None,
            resume_value: Some(value),
        }
    }

    /// Create a command that ends the graph immediately.
    pub fn end() -> Self {
        Self {
            update: None,
            goto: Some(CommandGoto::One(crate::END.to_string())),
            interrupt_value: None,
            resume_value: None,
        }
    }
}

/// Routing target for a Command.
#[derive(Debug, Clone)]
pub enum CommandGoto {
    /// Route to a single node.
    One(String),
    /// Fan-out to multiple nodes (map-reduce).
    Many(Vec<Send>),
}

/// What a node can return from its `process()` method.
///
/// Nodes can either return a simple state update (existing behavior)
/// or a `Command` for dynamic control flow.
#[derive(Clone)]
pub enum NodeOutput<S: State> {
    /// Regular state update (existing behavior).
    State(S),
    /// A command controlling flow + state.
    Command(Command<S>),
}

impl<S: State + std::fmt::Debug> std::fmt::Debug for NodeOutput<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeOutput::State(s) => f.debug_tuple("NodeOutput::State").field(s).finish(),
            NodeOutput::Command(c) => f.debug_tuple("NodeOutput::Command").field(c).finish(),
        }
    }
}

/// Blanket conversion: returning `S` from a node is shorthand for `NodeOutput::State(S)`.
impl<S: State> From<S> for NodeOutput<S> {
    fn from(state: S) -> Self {
        NodeOutput::State(state)
    }
}

/// The result of a graph invocation.
#[derive(Debug, Clone)]
pub enum GraphResult<S> {
    /// Graph completed successfully.
    Complete(S),
    /// Graph was interrupted and is waiting for input.
    Interrupted {
        state: S,
        /// The value passed to `interrupt()` — can be inspected by the caller.
        interrupt_value: Value,
    },
}

impl<S> GraphResult<S> {
    /// Get the state, regardless of whether the graph completed or was interrupted.
    pub fn state(&self) -> &S {
        match self {
            GraphResult::Complete(s) => s,
            GraphResult::Interrupted { state, .. } => state,
        }
    }

    /// Consume and return the state.
    pub fn into_state(self) -> S {
        match self {
            GraphResult::Complete(s) => s,
            GraphResult::Interrupted { state, .. } => state,
        }
    }

    /// Returns true if the graph completed normally.
    pub fn is_complete(&self) -> bool {
        matches!(self, GraphResult::Complete(_))
    }

    /// Returns true if the graph was interrupted.
    pub fn is_interrupted(&self) -> bool {
        matches!(self, GraphResult::Interrupted { .. })
    }

    /// Returns the interrupt value if the graph was interrupted.
    pub fn interrupt_value(&self) -> Option<&Value> {
        match self {
            GraphResult::Interrupted {
                interrupt_value, ..
            } => Some(interrupt_value),
            _ => None,
        }
    }
}

/// Interrupt graph execution and request human input.
///
/// This struct is stored in checkpoints for interrupted graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interrupt {
    pub value: Value,
}

/// Create an interrupt command that pauses graph execution.
///
/// The interrupt value is saved with the checkpoint and returned
/// to the caller as `GraphResult::Interrupted`. To resume, pass
/// `Command::resume(value)` as input to `graph.invoke()`.
///
/// # Example
///
/// ```ignore
/// use synaptic_graph::{interrupt, NodeOutput, MessageState};
///
/// async fn approval_node(state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
///     Ok(interrupt(serde_json::json!({"question": "Approve this action?"})))
/// }
/// ```
pub fn interrupt<S: State>(value: Value) -> NodeOutput<S> {
    NodeOutput::Command(Command {
        update: None,
        goto: None,
        interrupt_value: Some(value),
        resume_value: None,
    })
}
