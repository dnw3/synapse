use super::outbound::OutboundPayload;
use synaptic::DeliveryContext;

/// Response from AgentSession after processing a message.
#[allow(dead_code)]
pub struct AgentReply {
    /// Final accumulated response text.
    pub content: String,
    /// Structured outbound payloads for delivery adapters.
    pub payloads: Vec<OutboundPayload>,
    /// Resolved delivery target (where this reply should go).
    pub delivery_target: DeliveryContext,
    /// Turn identifier for tracing (same as envelope.request_id).
    pub turn_id: String,
}
