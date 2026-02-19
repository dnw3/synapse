use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::Stream;
use serde_json::Value;
use synaptic_core::SynapticError;
use tokio::sync::RwLock;

use crate::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};
use crate::command::{CommandGoto, GraphResult, NodeOutput};
use crate::edge::{ConditionalEdge, Edge};
use crate::node::Node;
use crate::state::State;
use crate::END;

/// Cache policy for node-level caching.
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Time-to-live for cached entries.
    pub ttl: Duration,
}

impl CachePolicy {
    /// Create a new cache policy with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self { ttl }
    }
}

/// Cached node output with expiry.
pub(crate) struct CachedEntry<S: State> {
    output: NodeOutput<S>,
    created: Instant,
    ttl: Duration,
}

impl<S: State> CachedEntry<S> {
    fn is_valid(&self) -> bool {
        self.created.elapsed() < self.ttl
    }
}

/// Hash a serializable state to use as a cache key.
fn hash_state(value: &Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let canonical = value.to_string();
    canonical.hash(&mut hasher);
    hasher.finish()
}

/// Controls what is yielded during graph streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    /// Yield full state after each node executes.
    Values,
    /// Yield only the delta (state before merge vs after, keyed by node name).
    Updates,
    /// Yield only AI messages from the state (useful for chat UIs).
    Messages,
    /// Yield detailed debug information including node timing.
    Debug,
    /// Yield custom events emitted via StreamWriter.
    Custom,
}

/// An event yielded during graph streaming.
#[derive(Debug, Clone)]
pub struct GraphEvent<S> {
    /// The node that just executed.
    pub node: String,
    /// The state snapshot (full state for Values mode, post-node state for Updates).
    pub state: S,
}

/// An event yielded during multi-mode streaming, tagged with its stream mode.
#[derive(Debug, Clone)]
pub struct MultiGraphEvent<S> {
    /// Which stream mode produced this event.
    pub mode: StreamMode,
    /// The underlying graph event.
    pub event: GraphEvent<S>,
}

/// A stream of graph events.
pub type GraphStream<'a, S> =
    Pin<Box<dyn Stream<Item = Result<GraphEvent<S>, SynapticError>> + Send + 'a>>;

/// A stream of multi-mode graph events.
pub type MultiGraphStream<'a, S> =
    Pin<Box<dyn Stream<Item = Result<MultiGraphEvent<S>, SynapticError>> + Send + 'a>>;

/// The compiled, executable graph.
pub struct CompiledGraph<S: State> {
    pub(crate) nodes: HashMap<String, Box<dyn Node<S>>>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) conditional_edges: Vec<ConditionalEdge<S>>,
    pub(crate) entry_point: String,
    pub(crate) interrupt_before: HashSet<String>,
    pub(crate) interrupt_after: HashSet<String>,
    pub(crate) checkpointer: Option<Arc<dyn Checkpointer>>,
    /// Cache policies keyed by node name.
    pub(crate) cache_policies: HashMap<String, CachePolicy>,
    /// Node-level cache: node_name -> (state_hash -> cached_output).
    pub(crate) cache: Arc<RwLock<HashMap<String, HashMap<u64, CachedEntry<S>>>>>,
    /// Nodes marked as deferred (wait for all incoming edges).
    pub(crate) deferred: HashSet<String>,
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

/// Internal helper: process a `NodeOutput` and return the next node to visit.
/// Returns `(next_node_or_none, interrupt_value_or_none)`.
fn handle_node_output<S: State>(
    output: NodeOutput<S>,
    state: &mut S,
    current_node: &str,
    find_next: impl Fn(&str, &S) -> String,
) -> (Option<String>, Option<serde_json::Value>) {
    match output {
        NodeOutput::State(new_state) => {
            *state = new_state;
            (None, None) // use normal routing
        }
        NodeOutput::Command(cmd) => {
            // Apply state update if present
            if let Some(update) = cmd.update {
                state.merge(update);
            }

            // Check for interrupt
            if let Some(interrupt_value) = cmd.interrupt_value {
                return (None, Some(interrupt_value));
            }

            // Determine routing
            match cmd.goto {
                Some(CommandGoto::One(target)) => (Some(target), None),
                Some(CommandGoto::Many(_sends)) => {
                    // Fan-out: for now, execute Send targets sequentially
                    // Full parallel execution is handled in the main loop
                    (Some("__fanout__".to_string()), None)
                }
                None => {
                    let next = find_next(current_node, state);
                    (Some(next), None)
                }
            }
        }
    }
}

/// Helper to serialize state into a checkpoint.
fn make_checkpoint<S: serde::Serialize>(
    state: &S,
    next_node: Option<String>,
    node_name: &str,
) -> Result<Checkpoint, SynapticError> {
    let state_val = serde_json::to_value(state)
        .map_err(|e| SynapticError::Graph(format!("serialize state: {e}")))?;
    Ok(Checkpoint::new(state_val, next_node).with_metadata("source", serde_json::json!(node_name)))
}

impl<S: State> CompiledGraph<S> {
    /// Set a checkpointer for state persistence.
    pub fn with_checkpointer(mut self, checkpointer: Arc<dyn Checkpointer>) -> Self {
        self.checkpointer = Some(checkpointer);
        self
    }

    /// Execute the graph with initial state.
    pub async fn invoke(&self, state: S) -> Result<GraphResult<S>, SynapticError>
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
    ) -> Result<GraphResult<S>, SynapticError>
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        // If there's a checkpoint, try to resume from it
        let mut resume_from: Option<String> = None;
        if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
            if let Some(checkpoint) = checkpointer.get(cfg).await? {
                state = serde_json::from_value(checkpoint.state).map_err(|e| {
                    SynapticError::Graph(format!("failed to deserialize checkpoint state: {e}"))
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
                return Err(SynapticError::Graph(
                    "max iterations (100) exceeded — possible infinite loop".to_string(),
                ));
            }
            max_iterations -= 1;

            // Check interrupt_before
            if self.interrupt_before.contains(&current_node) {
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    let checkpoint =
                        make_checkpoint(&state, Some(current_node.clone()), &current_node)?;
                    checkpointer.put(cfg, &checkpoint).await?;
                }
                return Ok(GraphResult::Interrupted {
                    state,
                    interrupt_value: serde_json::json!({
                        "reason": format!("interrupted before node '{current_node}'")
                    }),
                });
            }

            // Execute node (with optional cache)
            let node = self
                .nodes
                .get(&current_node)
                .ok_or_else(|| SynapticError::Graph(format!("node '{current_node}' not found")))?;
            let output = self
                .execute_with_cache(&current_node, node.as_ref(), state.clone())
                .await?;

            // Handle the output
            let (next_override, interrupt_value) =
                handle_node_output(output, &mut state, &current_node, |cur, s| {
                    self.find_next_node(cur, s)
                });

            // Check for interrupt from Command
            if let Some(interrupt_val) = interrupt_value {
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    let next = self.find_next_node(&current_node, &state);
                    let checkpoint = make_checkpoint(&state, Some(next), &current_node)?;
                    checkpointer.put(cfg, &checkpoint).await?;
                }
                return Ok(GraphResult::Interrupted {
                    state,
                    interrupt_value: interrupt_val,
                });
            }

            // Handle fan-out (Send)
            if next_override.as_deref() == Some("__fanout__") {
                // TODO: full parallel fan-out
                break;
            }

            let next = if let Some(target) = next_override {
                target
            } else {
                // Check interrupt_after (only when no command override)
                if self.interrupt_after.contains(&current_node) {
                    let next = self.find_next_node(&current_node, &state);
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        let checkpoint = make_checkpoint(&state, Some(next), &current_node)?;
                        checkpointer.put(cfg, &checkpoint).await?;
                    }
                    return Ok(GraphResult::Interrupted {
                        state,
                        interrupt_value: serde_json::json!({
                            "reason": format!("interrupted after node '{current_node}'")
                        }),
                    });
                }

                // Normal routing
                self.find_next_node(&current_node, &state)
            };

            // Save checkpoint after each node
            if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                let checkpoint = make_checkpoint(&state, Some(next.clone()), &current_node)?;
                checkpointer.put(cfg, &checkpoint).await?;
            }

            current_node = next;
        }

        Ok(GraphResult::Complete(state))
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
                                yield Err(SynapticError::Graph(format!(
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
                    yield Err(SynapticError::Graph(
                        "max iterations (100) exceeded — possible infinite loop".to_string(),
                    ));
                    return;
                }
                max_iterations -= 1;

                // Check interrupt_before
                if self.interrupt_before.contains(&current_node) {
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        match make_checkpoint(&state, Some(current_node.clone()), &current_node) {
                            Ok(checkpoint) => {
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
                    yield Err(SynapticError::Graph(format!(
                        "interrupted before node '{current_node}'"
                    )));
                    return;
                }

                // Execute node
                let node = match self.nodes.get(&current_node) {
                    Some(n) => n,
                    None => {
                        yield Err(SynapticError::Graph(format!("node '{current_node}' not found")));
                        return;
                    }
                };

                let output = match node.process(state.clone()).await {
                    Ok(o) => o,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                // Handle the node output
                let mut interrupt_val = None;
                let next_override = match output {
                    NodeOutput::State(new_state) => {
                        state = new_state;
                        None
                    }
                    NodeOutput::Command(cmd) => {
                        if let Some(update) = cmd.update {
                            state.merge(update);
                        }

                        if let Some(iv) = cmd.interrupt_value {
                            interrupt_val = Some(iv);
                            None
                        } else {
                            match cmd.goto {
                                Some(CommandGoto::One(target)) => Some(target),
                                Some(CommandGoto::Many(_)) => Some(END.to_string()),
                                None => None,
                            }
                        }
                    }
                };

                // Yield event
                let event = GraphEvent {
                    node: current_node.clone(),
                    state: state.clone(),
                };
                yield Ok(event);

                // Check for interrupt from Command
                if let Some(iv) = interrupt_val {
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        let next = self.find_next_node(&current_node, &state);
                        match make_checkpoint(&state, Some(next), &current_node) {
                            Ok(checkpoint) => {
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
                    yield Err(SynapticError::Graph(format!(
                        "interrupted by node '{current_node}': {iv}"
                    )));
                    return;
                }

                let next = if let Some(target) = next_override {
                    target
                } else {
                    // Check interrupt_after (only when no command override)
                    if self.interrupt_after.contains(&current_node) {
                        let next = self.find_next_node(&current_node, &state);
                        if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                            match make_checkpoint(&state, Some(next), &current_node) {
                                Ok(checkpoint) => {
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
                        yield Err(SynapticError::Graph(format!(
                            "interrupted after node '{current_node}'"
                        )));
                        return;
                    }

                    // Find next node via normal edge routing
                    self.find_next_node(&current_node, &state)
                };

                // Save checkpoint
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    match make_checkpoint(&state, Some(next.clone()), &current_node) {
                        Ok(checkpoint) => {
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

    /// Stream graph execution with multiple stream modes.
    ///
    /// Each event is tagged with the `StreamMode` that produced it.
    /// For a single node execution, one event per requested mode is emitted.
    pub fn stream_modes(&self, state: S, modes: Vec<StreamMode>) -> MultiGraphStream<'_, S>
    where
        S: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        self.stream_modes_with_config(state, modes, None)
    }

    /// Stream graph execution with multiple stream modes and optional checkpoint config.
    pub fn stream_modes_with_config(
        &self,
        state: S,
        modes: Vec<StreamMode>,
        config: Option<CheckpointConfig>,
    ) -> MultiGraphStream<'_, S>
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
                                yield Err(SynapticError::Graph(format!(
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
                    yield Err(SynapticError::Graph(
                        "max iterations (100) exceeded — possible infinite loop".to_string(),
                    ));
                    return;
                }
                max_iterations -= 1;

                // Check interrupt_before
                if self.interrupt_before.contains(&current_node) {
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        match make_checkpoint(&state, Some(current_node.clone()), &current_node) {
                            Ok(checkpoint) => {
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
                    yield Err(SynapticError::Graph(format!(
                        "interrupted before node '{current_node}'"
                    )));
                    return;
                }

                // Snapshot state before node execution (for Updates mode diff)
                let state_before = state.clone();

                // Execute node
                let node = match self.nodes.get(&current_node) {
                    Some(n) => n,
                    None => {
                        yield Err(SynapticError::Graph(format!("node '{current_node}' not found")));
                        return;
                    }
                };

                let output = match node.process(state.clone()).await {
                    Ok(o) => o,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                // Handle the node output
                let mut interrupt_val = None;
                let next_override = match output {
                    NodeOutput::State(new_state) => {
                        state = new_state;
                        None
                    }
                    NodeOutput::Command(cmd) => {
                        if let Some(update) = cmd.update {
                            state.merge(update);
                        }

                        if let Some(iv) = cmd.interrupt_value {
                            interrupt_val = Some(iv);
                            None
                        } else {
                            match cmd.goto {
                                Some(CommandGoto::One(target)) => Some(target),
                                Some(CommandGoto::Many(_)) => Some(END.to_string()),
                                None => None,
                            }
                        }
                    }
                };

                // Yield events for each requested mode
                for mode in &modes {
                    let event = match mode {
                        StreamMode::Values | StreamMode::Debug | StreamMode::Custom => {
                            // Full state after node execution
                            GraphEvent {
                                node: current_node.clone(),
                                state: state.clone(),
                            }
                        }
                        StreamMode::Updates => {
                            // State before node (the "delta" is the difference)
                            // For Updates, we yield the pre-node state so callers
                            // can diff against the full Values event
                            GraphEvent {
                                node: current_node.clone(),
                                state: state_before.clone(),
                            }
                        }
                        StreamMode::Messages => {
                            // Same as Values — callers filter for AI messages
                            GraphEvent {
                                node: current_node.clone(),
                                state: state.clone(),
                            }
                        }
                    };
                    yield Ok(MultiGraphEvent {
                        mode: *mode,
                        event,
                    });
                }

                // Check for interrupt from Command
                if let Some(iv) = interrupt_val {
                    if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                        let next = self.find_next_node(&current_node, &state);
                        match make_checkpoint(&state, Some(next), &current_node) {
                            Ok(checkpoint) => {
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
                    yield Err(SynapticError::Graph(format!(
                        "interrupted by node '{current_node}': {iv}"
                    )));
                    return;
                }

                let next = if let Some(target) = next_override {
                    target
                } else {
                    // Check interrupt_after (only when no command override)
                    if self.interrupt_after.contains(&current_node) {
                        let next = self.find_next_node(&current_node, &state);
                        if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                            match make_checkpoint(&state, Some(next), &current_node) {
                                Ok(checkpoint) => {
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
                        yield Err(SynapticError::Graph(format!(
                            "interrupted after node '{current_node}'"
                        )));
                        return;
                    }

                    // Find next node via normal edge routing
                    self.find_next_node(&current_node, &state)
                };

                // Save checkpoint
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    match make_checkpoint(&state, Some(next.clone()), &current_node) {
                        Ok(checkpoint) => {
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
    ) -> Result<(), SynapticError>
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapticError::Graph("no checkpointer configured".to_string()))?;

        let checkpoint = checkpointer
            .get(config)
            .await?
            .ok_or_else(|| SynapticError::Graph("no checkpoint found".to_string()))?;

        let mut current_state: S = serde_json::from_value(checkpoint.state)
            .map_err(|e| SynapticError::Graph(format!("deserialize: {e}")))?;

        current_state.merge(update);

        let updated = Checkpoint::new(
            serde_json::to_value(&current_state)
                .map_err(|e| SynapticError::Graph(format!("serialize: {e}")))?,
            checkpoint.next_node,
        )
        .with_metadata("source", serde_json::json!("update_state"));
        checkpointer.put(config, &updated).await?;

        Ok(())
    }

    /// Get the current state for a thread from the checkpointer.
    ///
    /// Returns `None` if no checkpoint exists for the given thread.
    pub async fn get_state(&self, config: &CheckpointConfig) -> Result<Option<S>, SynapticError>
    where
        S: serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapticError::Graph("no checkpointer configured".to_string()))?;

        match checkpointer.get(config).await? {
            Some(checkpoint) => {
                let state: S = serde_json::from_value(checkpoint.state).map_err(|e| {
                    SynapticError::Graph(format!("failed to deserialize checkpoint state: {e}"))
                })?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Get the state history for a thread (all checkpoints).
    ///
    /// Returns a list of `(state, next_node)` pairs, ordered from oldest to newest.
    pub async fn get_state_history(
        &self,
        config: &CheckpointConfig,
    ) -> Result<Vec<(S, Option<String>)>, SynapticError>
    where
        S: serde::de::DeserializeOwned,
    {
        let checkpointer = self
            .checkpointer
            .as_ref()
            .ok_or_else(|| SynapticError::Graph("no checkpointer configured".to_string()))?;

        let checkpoints = checkpointer.list(config).await?;
        let mut history = Vec::with_capacity(checkpoints.len());

        for checkpoint in checkpoints {
            let state: S = serde_json::from_value(checkpoint.state).map_err(|e| {
                SynapticError::Graph(format!("failed to deserialize checkpoint state: {e}"))
            })?;
            history.push((state, checkpoint.next_node));
        }

        Ok(history)
    }

    /// Execute a node, using cache if a CachePolicy is set for it.
    async fn execute_with_cache(
        &self,
        node_name: &str,
        node: &dyn Node<S>,
        state: S,
    ) -> Result<NodeOutput<S>, SynapticError>
    where
        S: serde::Serialize,
    {
        let policy = self.cache_policies.get(node_name);
        if policy.is_none() {
            return node.process(state).await;
        }
        let policy = policy.unwrap();

        // Compute state hash for cache key
        let state_val = serde_json::to_value(&state)
            .map_err(|e| SynapticError::Graph(format!("cache: serialize state: {e}")))?;
        let key = hash_state(&state_val);

        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(node_cache) = cache.get(node_name) {
                if let Some(entry) = node_cache.get(&key) {
                    if entry.is_valid() {
                        return Ok(entry.output.clone());
                    }
                }
            }
        }

        // Cache miss — execute the node
        let output = node.process(state).await?;

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            let node_cache = cache.entry(node_name.to_string()).or_default();
            node_cache.insert(
                key,
                CachedEntry {
                    output: output.clone(),
                    created: Instant::now(),
                    ttl: policy.ttl,
                },
            );
        }

        Ok(output)
    }

    /// Returns true if the given node is deferred (waits for all incoming paths).
    pub fn is_deferred(&self, node_name: &str) -> bool {
        self.deferred.contains(node_name)
    }

    /// Returns the number of incoming edges (fixed + conditional) for a node.
    pub fn incoming_edge_count(&self, node_name: &str) -> usize {
        let fixed = self.edges.iter().filter(|e| e.target == node_name).count();
        // Conditional edges may route to this node but we can't statically count them,
        // so we count the path_map entries that reference this node.
        let conditional = self
            .conditional_edges
            .iter()
            .filter_map(|ce| ce.path_map.as_ref())
            .flat_map(|pm| pm.values())
            .filter(|target| *target == node_name)
            .count();
        fixed + conditional
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
