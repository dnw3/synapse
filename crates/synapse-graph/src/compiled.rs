use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use synapse_core::SynapseError;

use crate::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};
use crate::edge::{ConditionalEdge, Edge};
use crate::node::Node;
use crate::state::State;
use crate::END;

/// The compiled, executable graph.
pub struct CompiledGraph<S: State> {
    pub(crate) nodes: HashMap<String, Box<dyn Node<S>>>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) conditional_edges: Vec<ConditionalEdge<S>>,
    pub(crate) entry_point: String,
    pub(crate) interrupt_before: HashSet<String>,
    pub(crate) interrupt_after: HashSet<String>,
    pub(crate) checkpointer: Option<Arc<dyn Checkpointer>>,
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

            // Check interrupt_after
            if self.interrupt_after.contains(&current_node) {
                // Find next node first so we can save it
                let next = self.find_next_node(&current_node, &state);
                if let (Some(ref checkpointer), Some(ref cfg)) = (&self.checkpointer, &config) {
                    let checkpoint = Checkpoint {
                        state: serde_json::to_value(&state)
                            .map_err(|e| SynapseError::Graph(format!("serialize state: {e}")))?,
                        next_node: Some(next),
                    };
                    checkpointer.put(cfg, &checkpoint).await?;
                }
                return Err(SynapseError::Graph(format!(
                    "interrupted after node '{current_node}'"
                )));
            }

            // Find next node
            let next = self.find_next_node(&current_node, &state);

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
