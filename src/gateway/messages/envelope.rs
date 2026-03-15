use serde::{Deserialize, Serialize};
use synaptic::{DeliveryContext, InputProvenance, ProvenanceKind};

use crate::gateway::presence::now_ms;
use crate::logging;

/// File attachment from any channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub url: String,
    pub mime_type: Option<String>,
}

/// Unified message envelope for all channels.
pub struct MessageEnvelope {
    pub request_id: String,
    pub session_key: String,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub delivery: DeliveryContext,
    pub provenance: InputProvenance,
    pub idempotency_key: Option<String>,
    pub timestamp_ms: u64,
    /// Sender identity (user ID) for routing and session isolation.
    pub sender_id: Option<String>,
    /// Routing metadata for multi-agent dispatch.
    pub routing: RoutingMeta,
}

/// Routing metadata carried in the envelope for multi-agent dispatch.
#[derive(Debug, Default, Clone)]
pub struct RoutingMeta {
    /// Discord guild ID.
    pub guild_id: Option<String>,
    /// Slack team/workspace ID.
    pub team_id: Option<String>,
    /// User roles (Discord).
    pub roles: Vec<String>,
    /// Whether this is a DM or group.
    pub peer_kind: Option<crate::config::PeerKind>,
    /// Primary peer ID (chat_id for groups, sender_id for DMs).
    pub peer_id: Option<String>,
}

#[allow(dead_code)]
impl MessageEnvelope {
    /// Create a new envelope with minimal fields. Sets provenance to ExternalUser.
    pub fn new(request_id: String, session_key: String, content: String) -> Self {
        Self {
            request_id,
            session_key,
            content,
            attachments: Vec::new(),
            delivery: DeliveryContext::default(),
            provenance: InputProvenance::default(),
            idempotency_key: None,
            timestamp_ms: now_ms(),
            sender_id: None,
            routing: RoutingMeta::default(),
        }
    }

    /// Create an envelope for the webchat (web UI) channel.
    pub fn webchat(
        request_id: String,
        session_key: String,
        content: String,
        conn_id: &str,
    ) -> Self {
        Self {
            request_id,
            session_key,
            content,
            attachments: Vec::new(),
            delivery: DeliveryContext {
                channel: "webchat".into(),
                to: Some(format!("conn:{}", conn_id)),
                ..Default::default()
            },
            provenance: InputProvenance {
                kind: ProvenanceKind::ExternalUser,
                source_channel: Some("webchat".into()),
                ..Default::default()
            },
            idempotency_key: None,
            timestamp_ms: now_ms(),
            sender_id: None,
            routing: RoutingMeta::default(),
        }
    }

    /// Create an envelope for a bot channel.
    pub fn channel(session_key: String, content: String, delivery: DeliveryContext) -> Self {
        let channel = delivery.channel.clone();
        Self {
            request_id: logging::generate_request_id(),
            session_key,
            content,
            attachments: Vec::new(),
            delivery,
            provenance: InputProvenance {
                kind: ProvenanceKind::ExternalUser,
                source_channel: Some(channel),
                ..Default::default()
            },
            idempotency_key: None,
            timestamp_ms: now_ms(),
            sender_id: None,
            routing: RoutingMeta::default(),
        }
    }
}
