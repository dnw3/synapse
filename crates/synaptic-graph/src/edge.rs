use std::collections::HashMap;
use std::sync::Arc;

use crate::State;

/// A fixed edge from source node to target node.
#[derive(Debug, Clone)]
pub struct Edge {
    pub source: String,
    pub target: String,
}

/// A routing function that inspects state and returns a target node name.
pub type RouterFn<S> = Arc<dyn Fn(&S) -> String + Send + Sync>;

/// A conditional edge from source node to a dynamically chosen target.
pub struct ConditionalEdge<S: State> {
    pub source: String,
    pub router: RouterFn<S>,
    /// Optional mapping of label → target node name, used for visualization.
    pub path_map: Option<HashMap<String, String>>,
}
