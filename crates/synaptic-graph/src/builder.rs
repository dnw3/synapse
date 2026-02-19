use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use synaptic_core::SynapticError;
use tokio::sync::RwLock;

use crate::compiled::{CachePolicy, CompiledGraph};
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
    cache_policies: HashMap<String, CachePolicy>,
    deferred: HashSet<String>,
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
            cache_policies: HashMap::new(),
            deferred: HashSet::new(),
        }
    }

    /// Add a named node to the graph.
    pub fn add_node(mut self, name: impl Into<String>, node: impl Node<S> + 'static) -> Self {
        self.nodes.insert(name.into(), Box::new(node));
        self
    }

    /// Add a deferred node that waits until ALL incoming edges have been
    /// traversed before executing. Useful for fan-in aggregation after
    /// parallel fan-out with [`Send`](crate::Send).
    pub fn add_deferred_node(
        mut self,
        name: impl Into<String>,
        node: impl Node<S> + 'static,
    ) -> Self {
        let n = name.into();
        self.nodes.insert(n.clone(), Box::new(node));
        self.deferred.insert(n);
        self
    }

    /// Add a named node with caching. Results are cached based on
    /// a hash of the serialized input state for the duration of the TTL.
    pub fn add_node_with_cache(
        mut self,
        name: impl Into<String>,
        node: impl Node<S> + 'static,
        cache: CachePolicy,
    ) -> Self {
        let n = name.into();
        self.nodes.insert(n.clone(), Box::new(node));
        self.cache_policies.insert(n, cache);
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
            path_map: None,
        });
        self
    }

    /// Add a conditional edge with a routing function and a path map for visualization.
    pub fn add_conditional_edges_with_path_map(
        mut self,
        source: impl Into<String>,
        router: impl Fn(&S) -> String + Send + Sync + 'static,
        path_map: HashMap<String, String>,
    ) -> Self {
        self.conditional_edges.push(ConditionalEdge {
            source: source.into(),
            router: Arc::new(router),
            path_map: Some(path_map),
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
    pub fn compile(self) -> Result<CompiledGraph<S>, SynapticError> {
        let entry = self
            .entry_point
            .ok_or_else(|| SynapticError::Graph("no entry point set".to_string()))?;

        if !self.nodes.contains_key(&entry) {
            return Err(SynapticError::Graph(format!(
                "entry point node '{entry}' not found"
            )));
        }

        // Validate: every edge references existing nodes or END
        for edge in &self.edges {
            if edge.source != START && !self.nodes.contains_key(&edge.source) {
                return Err(SynapticError::Graph(format!(
                    "edge source '{}' not found",
                    edge.source
                )));
            }
            if edge.target != END && !self.nodes.contains_key(&edge.target) {
                return Err(SynapticError::Graph(format!(
                    "edge target '{}' not found",
                    edge.target
                )));
            }
        }

        for ce in &self.conditional_edges {
            if ce.source != START && !self.nodes.contains_key(&ce.source) {
                return Err(SynapticError::Graph(format!(
                    "conditional edge source '{}' not found",
                    ce.source
                )));
            }
            // Validate path_map targets reference existing nodes or END
            if let Some(ref path_map) = ce.path_map {
                for (label, target) in path_map {
                    if target != END && !self.nodes.contains_key(target) {
                        return Err(SynapticError::Graph(format!(
                            "conditional edge path_map target '{target}' (label '{label}') not found"
                        )));
                    }
                }
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
            cache_policies: self.cache_policies,
            cache: Arc::new(RwLock::new(HashMap::new())),
            deferred: self.deferred,
        })
    }
}

impl<S: State> Default for StateGraph<S> {
    fn default() -> Self {
        Self::new()
    }
}
