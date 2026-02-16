use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use synapse_core::SynapseError;

use crate::compiled::CompiledGraph;
use crate::edge::{ConditionalEdge, Edge};
use crate::node::Node;
use crate::state::State;
use crate::{END, START};

/// Builder for constructing a state graph.
pub struct StateGraph<S: State> {
    nodes: HashMap<String, Box<dyn Node<S>>>,
    edges: Vec<Edge>,
    conditional_edges: Vec<ConditionalEdge<S>>,
    entry_point: Option<String>,
    interrupt_before: HashSet<String>,
    interrupt_after: HashSet<String>,
}

impl<S: State> StateGraph<S> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            conditional_edges: Vec::new(),
            entry_point: None,
            interrupt_before: HashSet::new(),
            interrupt_after: HashSet::new(),
        }
    }

    /// Add a named node to the graph.
    pub fn add_node(mut self, name: impl Into<String>, node: impl Node<S> + 'static) -> Self {
        self.nodes.insert(name.into(), Box::new(node));
        self
    }

    /// Add a fixed edge from source to target.
    pub fn add_edge(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.edges.push(Edge {
            source: source.into(),
            target: target.into(),
        });
        self
    }

    /// Add a conditional edge with a routing function.
    pub fn add_conditional_edges(
        mut self,
        source: impl Into<String>,
        router: impl Fn(&S) -> String + Send + Sync + 'static,
    ) -> Self {
        self.conditional_edges.push(ConditionalEdge {
            source: source.into(),
            router: Arc::new(router),
        });
        self
    }

    /// Set the entry point node for graph execution.
    pub fn set_entry_point(mut self, name: impl Into<String>) -> Self {
        self.entry_point = Some(name.into());
        self
    }

    /// Mark nodes that should interrupt BEFORE execution (human-in-the-loop).
    pub fn interrupt_before(mut self, nodes: Vec<String>) -> Self {
        self.interrupt_before.extend(nodes);
        self
    }

    /// Mark nodes that should interrupt AFTER execution (human-in-the-loop).
    pub fn interrupt_after(mut self, nodes: Vec<String>) -> Self {
        self.interrupt_after.extend(nodes);
        self
    }

    /// Compile the graph into an executable CompiledGraph.
    pub fn compile(self) -> Result<CompiledGraph<S>, SynapseError> {
        let entry = self
            .entry_point
            .ok_or_else(|| SynapseError::Graph("no entry point set".to_string()))?;

        if !self.nodes.contains_key(&entry) {
            return Err(SynapseError::Graph(format!(
                "entry point node '{entry}' not found"
            )));
        }

        // Validate: every edge references existing nodes or END
        for edge in &self.edges {
            if edge.source != START && !self.nodes.contains_key(&edge.source) {
                return Err(SynapseError::Graph(format!(
                    "edge source '{}' not found",
                    edge.source
                )));
            }
            if edge.target != END && !self.nodes.contains_key(&edge.target) {
                return Err(SynapseError::Graph(format!(
                    "edge target '{}' not found",
                    edge.target
                )));
            }
        }

        for ce in &self.conditional_edges {
            if ce.source != START && !self.nodes.contains_key(&ce.source) {
                return Err(SynapseError::Graph(format!(
                    "conditional edge source '{}' not found",
                    ce.source
                )));
            }
        }

        Ok(CompiledGraph {
            nodes: self.nodes,
            edges: self.edges,
            conditional_edges: self.conditional_edges,
            entry_point: entry,
            interrupt_before: self.interrupt_before,
            interrupt_after: self.interrupt_after,
            checkpointer: None,
        })
    }
}

impl<S: State> Default for StateGraph<S> {
    fn default() -> Self {
        Self::new()
    }
}
