use serde::Serialize;
use synaptic::ProvenanceKind;

#[derive(Debug, Clone, Serialize)]
pub struct MessageReceivedEvent {
    pub request_id: String,
    pub session_key: String,
    pub channel: String,
    pub to: Option<String>,
    pub provenance: ProvenanceKind,
    pub timestamp_ms: u64,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageSentEvent {
    pub request_id: String,
    pub channel: String,
    pub to: Option<String>,
    pub timestamp_ms: u64,
    pub message_id: Option<String>,
}

impl MessageReceivedEvent {
    /// Create from an `InboundMessage`.
    pub fn from_inbound(msg: &super::InboundMessage) -> Self {
        let preview = if msg.content.len() > 100 {
            format!("{}...", &msg.content[..100])
        } else {
            msg.content.clone()
        };
        Self {
            request_id: msg.request_id.clone(),
            session_key: msg.session_key.clone(),
            channel: msg.channel.platform.clone(),
            to: msg.sender.id.clone(),
            provenance: ProvenanceKind::ExternalUser,
            timestamp_ms: msg.timestamp_ms,
            content_preview: preview,
        }
    }
}
