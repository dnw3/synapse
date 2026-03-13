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
    pub fn from_envelope(envelope: &super::MessageEnvelope) -> Self {
        let preview = if envelope.content.len() > 100 {
            format!("{}...", &envelope.content[..100])
        } else {
            envelope.content.clone()
        };
        Self {
            request_id: envelope.request_id.clone(),
            session_key: envelope.session_key.clone(),
            channel: envelope.delivery.channel.clone(),
            to: envelope.delivery.to.clone(),
            provenance: envelope.provenance.kind.clone(),
            timestamp_ms: envelope.timestamp_ms,
            content_preview: preview,
        }
    }
}
