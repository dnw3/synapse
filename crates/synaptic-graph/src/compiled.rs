use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use synaptic_core::SynapseError;

use crate::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};
use crate::command::{GraphCommand, GraphContext};
use crate::edge::{ConditionalEdge, Edge};
use crate::node::Node;
use crate::state::State;
use crate::END;

/// Controls what is yielded during graph streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    /// Yield full state after each node executes.
    Values,
    /// Yield only the delta (state before merge vs after, keyed by node name).
    Updates,
}

/// An event yielded during graph streaming.
#[derive(Debug, Clone)]
pub struct GraphEvent<S> {
    /// The node that just executed.
    pub node: String,
    /// The state snapshot (full state for Values mode, post-node state for Updates).
    pub state: S,
}

/// A stream of graph events.
pub type GraphStream<'a, S> =
    Pin<Box<dyn Stream<Item = Result<GraphEvent<S>, SynapseError>> + Send + 'a>>;

/// The compiled, executable graph.
pub struct CompiledGraph<S: State> {
    pub(crate) nodes: HashMap<String, Box<dyn Node<S>>>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) conditional_edges: Vec<ConditionalEdge<S>>,
    pub(crate) entry_point: String,
    pub(crate) interrupt_before: HashSet<String>,
    pub(crate) interrupt_after: HashSet<String>,
    pub(crate) checkpointer: Option<Arc<dyn Checkpointer>>,
    pub(crate) command_context: GraphContext,
}

impl<S: State> std::fmt::Debug for CompiledGraph<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledGraph")
            .field("entry_point", &self.entry_point)
            .field("node_count", &self.nodes.len())
            .field("edge_count", &self.edges.len())
            .field("conditional_edge_count", &self.conditional_edges.len())
            .finish()
    }
}

impl<S: State> CompiledGraph<S> {
    /// Set a checkpointer for state persistence.
    pub fn with_checkpointer(mut self, checkpointer: Arc<dyn Checkpointer>) -> Self {
        self.checkpointer = Some(checkpointer);
        self
    }

    /// Get the `GraphContext` for this compiled graph.
    ///
    /// Nodes can use this context to issue dynamic control flow commands
    /// (e.g., `goto` or `end`) that override normal edge-based routing.
    pub fn context(&self) -> &GraphContext {
        &self.command_context
    }

    /// Execute the graph with initial state.
    pub async fn invoke(&self, state: S) -> Result<S, SynapseError>
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        self.invoke_with_config(state, None).await
    }

    /// Execute with optional checkpoint config for resumption.
    pub async fn invoke_with_config(
        &self,
        mut state: S,
        config: Option<CheckpointConfig>,
    ) -> Result<S, SynapseError>
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        // If there's a checkpoint, try to resume from it
        let mut resume_from: Option<String> = None;
        if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
            if let Some(checkpoint) = checkpointer.get(cfg).await? {
                state = serde_json::from_value(checkpoint.state).map_err(|e| {
                    SynapseError::Graph(format!("failed to deserialize checkpoint state: {e}"))
                })?;
                resume_from = checkpoint.next_node;
            }
        }

        let mut current_node = resume_from.unwrap_or_else(|| self.entry_point.clone());
        let mut max_iterations = 100; // safety guard

        loop {
            if current_node == END {
                break;
            }
            if max_iterations == 0 {
                return Err(SynapseError::Graph(
                    "max iterations (100) exceeded — possible infinite loop".to_string(),
                ));
            }
            max_iterations -= 1;

            // Check interrupt_before
            if self.interrupt_before.contains(&current_node) {
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    let checkpoint = Checkpoint {
                        state: serde_json::to_value(&state)
                            .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")))?,
                        next_node: Some(current_node.clone()),
                    };
                    checkpointer.put(cfg, &checkpoint).await?;
                }
                return Err(SynapseError::Graph(format!(
                    "interrupted before node '{current_node}'"
                )));
            }

            // Execute node
            let node = self
                .nodes
                .get(&current_node)
                .ok_or_else(|| SynapseError::Graph(format!("node '{current_node}' not found")))?;
            state = node.process(state).await?;

            // Check for command from GraphContext
            let next = if let Some(cmd) = self.command_context.take_command().await {
                match cmd {
                    GraphCommand::Goto(target) => target,
                    GraphCommand::End => END.to_string(),
                }
            } else {
                // Check interrupt_after (only when no command override)
                if self.interrupt_after.contains(&current_node) {
                    // Find next node first so we can save it
                    let next = self.find_next_node(&current_node, &state);
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        let checkpoint = Checkpoint {
                            state: serde_json::to_value(&state).map_err(|e| {
                                SynapseError::Graph(format!("serialize state: {e}"))
                            })?,
                            next_node: Some(next),
                        };
                        checkpointer.put(cfg, &checkpoint).await?;
                    }
                    return Err(SynapseError::Graph(format!(
                        "interrupted after node '{current_node}'"
                    )));
                }

                // Find next node via normal edge routing
                self.find_next_node(&current_node, &state)
            };

            // Save checkpoint after each node
            if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                let checkpoint = Checkpoint {
                    state: serde_json::to_value(&state)
                        .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")))?,
                    next_node: Some(next.clone()),
                };
                checkpointer.put(cfg, &checkpoint).await?;
            }

            current_node = next;
        }

        Ok(state)
    }

    /// Stream graph execution, yielding a `GraphEvent` after each node.
    pub fn stream(&self, state: S, mode: StreamMode) -> GraphStream<'_, S>
    where
        S: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        self.stream_with_config(state, mode, None)
    }

    /// Stream graph execution with optional checkpoint config.
    pub fn stream_with_config(
        &self,
        state: S,
        _mode: StreamMode,
        config: Option<CheckpointConfig>,
    ) -> GraphStream<'_, S>
    where
        S: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        Box::pin(async_stream::stream! {
            let mut state = state;

            // If there's a checkpoint, try to resume from it
            let mut resume_from: Option<String> = None;
            if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                match checkpointer.get(cfg).await {
                    Ok(Some(checkpoint)) => {
                        match serde_json::from_value(checkpoint.state) {
                            Ok(s) => {
                                state = s;
                                resume_from = checkpoint.next_node;
                            }
                            Err(e) => {
                                yield Err(SynapseError::Graph(format!(
                                    "failed to deserialize checkpoint state: {e}"
                                )));
                                return;
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }

            let mut current_node = resume_from.unwrap_or_else(|| self.entry_point.clone());
            let mut max_iterations = 100;

            loop {
                if current_node == END {
                    break;
                }
                if max_iterations == 0 {
                    yield Err(SynapseError::Graph(
                        "max iterations (100) exceeded — possible infinite loop".to_string(),
                    ));
                    return;
                }
                max_iterations -= 1;

                // Check interrupt_before
                if self.interrupt_before.contains(&current_node) {
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        let ckpt_result = serde_json::to_value(&state)
                            .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")));
                        match ckpt_result {
                            Ok(state_val) => {
                                let checkpoint = Checkpoint {
                                    state: state_val,
                                    next_node: Some(current_node.clone()),
                                };
                                if let Err(e) = checkpointer.put(cfg, &checkpoint).await {
                                    yield Err(e);
                                    return;
                                }
                            }
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }
                    yield Err(SynapseError::Graph(format!(
                        "interrupted before node '{current_node}'"
                    )));
                    return;
                }

                // Execute node
                let node = match self.nodes.get(&current_node) {
                    Some(n) => n,
                    None => {
                        yield Err(SynapseError::Graph(format!("node '{current_node}' not found")));
                        return;
                    }
                };

                match node.process(state.clone()).await {
                    Ok(new_state) => {
                        state = new_state;
                    }
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }

                // Yield event
                let event = GraphEvent {
                    node: current_node.clone(),
                    state: state.clone(),
                };
                yield Ok(event);

                // Check for command from GraphContext
                let next = if let Some(cmd) = self.command_context.take_command().await {
                    match cmd {
                        GraphCommand::Goto(target) => target,
                        GraphCommand::End => END.to_string(),
                    }
                } else {
                    // Check interrupt_after (only when no command override)
                    if self.interrupt_after.contains(&current_node) {
                        let next = self.find_next_node(&current_node, &state);
                        if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                            let ckpt_result = serde_json::to_value(&state)
                                .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")));
                            match ckpt_result {
                                Ok(state_val) => {
                                    let checkpoint = Checkpoint {
                                        state: state_val,
                                        next_node: Some(next),
                                    };
                                    if let Err(e) = checkpointer.put(cfg, &checkpoint).await {
                                        yield Err(e);
                                        return;
                                    }
                                }
                                Err(e) => {
                                    yield Err(e);
                                    return;
                                }
                            }
                        }
                        yield Err(SynapseError::Graph(format!(
                            "interrupted after node '{current_node}'"
                        )));
                        return;
                    }

                    // Find next node via normal edge routing
                    self.find_next_node(&current_node, &state)
                };

                // Save checkpoint
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    let ckpt_result = serde_json::to_value(&state)
                        .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")));
                    match ckpt_result {
                        Ok(state_val) => {
                            let checkpoint = Checkpoint {
                                state: state_val,
                                next_node: Some(next.clone()),
                            };
                            if let Err(e) = checkpointer.put(cfg, &checkpoint).await {
                                yield Err(e);
                                return;
                            }
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                }

                current_node = next;
            }
        })
    }

    /// Update state on an interrupted graph (for human-in-the-loop).
    pub async fn update_state(
        &self,
        config: &CheckpointConfig,
        update: S,
    ) -> Result<(), SynapseError>
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapseError::Graph("no checkpointer configured".to_string()))?;

        let checkpoint = checkpointer
            .get(config)
            .await?
            .ok_or_else(|| SynapseError::Graph("no checkpoint found".to_string()))?;

        let mut current_state: S = serde_json::from_value(checkpoint.state)
            .map_err(|e| SynapseError::Graph(format!("deserialize: {e}")))?;

        current_state.merge(update);

        let updated = Checkpoint {
            state: serde_json::to_value(&current_state)
                .map_err(|e| SynapseError::Graph(format!("serialize: {e}")))?,
            next_node: checkpoint.next_node,
        };
        checkpointer.put(config, &updated).await?;

        Ok(())
    }

    /// Get the current state for a thread from the checkpointer.
    ///
    /// Returns `None` if no checkpoint exists for the given thread.
    pub async fn get_state(&self, config: &CheckpointConfig) -> Result<Option<S>, SynapseError>
    where
        S: serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapseError::Graph("no checkpointer configured".to_string()))?;

        match checkpointer.get(config).await? {
            Some(checkpoint) => {
                let state: S = serde_json::from_value(checkpoint.state).map_err(|e| {
                    SynapseError::Graph(format!("failed to deserialize checkpoint state: {e}"))
                })?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Get the state history for a thread (all checkpoints).
    ///
    /// Returns a list of `(state, next_node)` pairs, ordered from oldest to newest.
    /// The `next_node` indicates which node was scheduled to execute next when
    /// the checkpoint was saved.
    pub async fn get_state_history(
        &self,
        config: &CheckpointConfig,
    ) -> Result<Vec<(S, Option<String>)>, SynapseError>
    where
        S: serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapseError::Graph("no checkpointer configured".to_string()))?;

        let checkpoints = checkpointer.list(config).await?;
        let mut history = Vec::with_capacity(checkpoints.len());

        for checkpoint in checkpoints {
            let state: S = serde_json::from_value(checkpoint.state).map_err(|e| {
                SynapseError::Graph(format!("failed to deserialize checkpoint state: {e}"))
            })?;
            history.push((state, checkpoint.next_node));
        }

        Ok(history)
    }

    fn find_next_node(&self, current: &str, state: &S) -> String {
        // Check conditional edges first
        for ce in &self.conditional_edges {
            if ce.source == current {
                return (ce.router)(state);
            }
        }

        // Check fixed edges
        for edge in &self.edges {
            if edge.source == current {
                return edge.target.clone();
            }
        }

        // No outgoing edge means END
        END.to_string()
    }
}
