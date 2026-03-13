use synaptic::DeliveryContext;

/// Response from AgentSession after processing a message.
pub struct AgentReply {
    /// Final accumulated response text.
    pub content: String,
    /// Resolved delivery target (where this reply should go).
    pub delivery_target: DeliveryContext,
    /// Turn identifier for tracing (same as envelope.request_id).
    pub turn_id: String,
}
