use std::sync::Arc;

use tokio::sync::Mutex;

/// A graph execution command that can override normal edge routing.
///
/// Commands provide dynamic control flow within graph nodes, allowing
/// nodes to redirect execution to specific nodes or end the graph
/// without relying solely on edge definitions.
#[derive(Debug, Clone)]
pub enum GraphCommand {
    /// Go to a specific node next, overriding normal routing.
    Goto(String),
    /// End the graph execution immediately.
    End,
}

/// A context that nodes can use to issue graph execution commands.
///
/// The `GraphContext` is shared between the graph execution loop and
/// individual nodes. Nodes call `goto()` or `end()` to signal the
/// desired control flow, and the compiled graph checks for these
/// commands after each node execution.
///
/// # Example
///
/// ```ignore
/// use synaptic_graph::GraphContext;
///
/// async fn my_node_logic(ctx: &GraphContext) {
///     // Skip normal routing and go directly to "summary" node
///     ctx.goto("summary").await;
/// }
/// ```
#[derive(Clone)]
pub struct GraphContext {
    command: Arc<Mutex<Option<GraphCommand>>>,
}

impl GraphContext {
    /// Create a new `GraphContext`.
    pub fn new() -> Self {
        Self {
            command: Arc::new(Mutex::new(None)),
        }
    }

    /// Signal the graph to go to a specific node next, overriding normal routing.
    pub async fn goto(&self, node: impl Into<String>) {
        *self.command.lock().await = Some(GraphCommand::Goto(node.into()));
    }

    /// Signal the graph to end execution immediately.
    pub async fn end(&self) {
        *self.command.lock().await = Some(GraphCommand::End);
    }

    /// Take the current command (if any), clearing it from the slot.
    ///
    /// This is used internally by `CompiledGraph` after each node execution.
    pub(crate) async fn take_command(&self) -> Option<GraphCommand> {
        self.command.lock().await.take()
    }
}

impl Default for GraphContext {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GraphContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphContext").finish()
    }
}
