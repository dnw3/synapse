/// A Send instruction for dynamic fan-out to multiple nodes.
///
/// In LangGraph, `Send` allows conditional edges to dispatch work to
/// multiple nodes in parallel, each receiving a different state payload.
/// This is useful for map-reduce patterns where a single node's output
/// needs to be processed by multiple downstream nodes concurrently.
///
/// This is currently a placeholder type for future fan-out support.
/// The type is defined and exported so that user code can start
/// referencing it, but the actual parallel dispatch is not yet
/// implemented in `CompiledGraph`.
///
/// # Example (future API)
///
/// ```ignore
/// use synaptic_graph::Send;
///
/// // In a conditional edge router, return multiple Send instructions
/// // to fan out to different nodes with different state payloads:
/// let sends = vec![
///     Send::new("process_chunk", serde_json::json!({"chunk": "part1"})),
///     Send::new("process_chunk", serde_json::json!({"chunk": "part2"})),
/// ];
/// ```
#[derive(Debug, Clone)]
pub struct Send {
    /// The target node to send work to.
    pub node: String,
    /// The state payload to pass to the target node (serialized as JSON).
    pub state: serde_json::Value,
}

impl Send {
    /// Create a new `Send` instruction.
    ///
    /// # Arguments
    ///
    /// * `node` - The name of the target node to send work to.
    /// * `state` - The state payload (as a `serde_json::Value`) to pass.
    pub fn new(node: impl Into<String>, state: serde_json::Value) -> Self {
        Self {
            node: node.into(),
            state,
        }
    }
}
