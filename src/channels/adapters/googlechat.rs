use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Json;
use axum::Router;
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

/// Shared state for the axum webhook server.
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
}

/// Incoming Google Chat event payload (subset of fields we care about).
///
/// Reference: https://developers.google.com/chat/api/reference/rest/v1/Event
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GoogleChatEvent {
    /// Event type: "MESSAGE", "ADDED_TO_SPACE", "REMOVED_FROM_SPACE", "CARD_CLICKED", etc.
    #[serde(rename = "type")]
    event_type: Option<String>,
    /// The message that triggered the event (present for MESSAGE events).
    message: Option<ChatMessage>,
    /// The space in which the event occurred.
    space: Option<Space>,
}

/// A Google Chat message object (subset).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ChatMessage {
    /// The resource name of the message (e.g. "spaces/xxx/messages/yyy").
    name: Option<String>,
    /// Plain-text body of the message.
    text: Option<String>,
    /// The user who sent the message.
    sender: Option<User>,
}

/// A Google Chat user object.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct User {
    /// Resource name of the user (e.g. "users/12345").
    name: Option<String>,
    /// Display name of the user.
    display_name: Option<String>,
}

/// A Google Chat space object.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Space {
    /// Resource name of the space (e.g. "spaces/AAAA1234").
    name: Option<String>,
    /// Display name of the space.
    display_name: Option<String>,
}

/// Synchronous reply body returned to Google Chat.
///
/// Google Chat reads the HTTP response body as the bot reply when
/// the bot handles events synchronously (within 30 seconds).
#[derive(Debug, serde::Serialize)]
struct TextReply {
    text: String,
}

/// Handle incoming Google Chat webhook POST at `/`.
///
/// Google Chat sends all event types (MESSAGE, ADDED_TO_SPACE, etc.) to the
/// same endpoint. We only act on MESSAGE events; everything else returns 200 OK
/// with an empty body so Google Chat does not report an error.
async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    Json(event): Json<GoogleChatEvent>,
) -> Result<Json<TextReply>, StatusCode> {
    // Only process MESSAGE events.
    let event_type = event.event_type.as_deref().unwrap_or("");
    if event_type != "MESSAGE" {
        return Err(StatusCode::OK);
    }

    let message = match event.message.as_ref() {
        Some(m) => m,
        None => return Err(StatusCode::OK),
    };

    let text = match message.text.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return Err(StatusCode::OK),
    };

    let sender_name = message
        .sender
        .as_ref()
        .and_then(|u| u.name.as_deref())
        .map(|s| s.to_string());

    let space_name = event
        .space
        .as_ref()
        .and_then(|s| s.name.as_deref())
        .map(|s| s.to_string());

    // Allowlist check: filter by sender resource name or space resource name.
    if !state
        .allowlist
        .is_allowed(sender_name.as_deref(), space_name.as_deref())
    {
        // Silently ignore — return 200 with no text so Google Chat doesn't retry.
        return Err(StatusCode::OK);
    }

    // Session key scoped to the space (group chat) or sender (DM).
    let session_key = if let Some(ref space) = space_name {
        format!("googlechat:{}", space)
    } else if let Some(ref sender) = sender_name {
        format!("googlechat:{}", sender)
    } else {
        "googlechat:unknown".to_string()
    };

    // Determine peer kind: if no space, it's a DM.
    let is_dm = space_name.is_none();

    // Process the message through the agent session.
    let channel_info = ChannelInfo {
        platform: "googlechat".into(),
        native_channel_id: space_name.clone(),
        ..Default::default()
    };
    let sender_info = SenderInfo {
        id: sender_name.clone(),
        ..Default::default()
    };
    let chat_info = ChatInfo {
        chat_type: if is_dm { "direct" } else { "group" }.to_string(),
        ..Default::default()
    };
    let mut msg = InboundMessage::channel(
        session_key.clone(),
        text.clone(),
        channel_info,
        sender_info,
        chat_info,
    );
    msg.finalize();
    let reply_text = match state
        .agent_session
        .handle_message(msg, RunContext::default())
        .await
    {
        Ok(reply) => {
            let chunks = formatter::format_for_channel(&reply.content, "googlechat", 4096);
            // Google Chat synchronous replies support only a single text body.
            // If the response is chunked, join all chunks separated by a blank line.
            if chunks.is_empty() {
                reply.content
            } else {
                chunks.join("\n\n")
            }
        }
        Err(e) => {
            tracing::error!(channel = "googlechat", error = %e, "handler error");
            format!("Error: {}", e)
        }
    };

    Ok(Json(TextReply { text: reply_text }))
}

/// Run the Google Chat bot adapter (webhook mode).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let gchat_configs: Vec<crate::config::GoogleChatBotConfig> = config.channel_configs("googlechat");
    let gchat_config = gchat_configs
        .first()
        .ok_or("missing [[channels.googlechat]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = gchat_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = gchat_config.port.unwrap_or(8077);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "googlechat",
            senders = allowlist.allowed_users.len(),
            spaces = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    if let Some(ref project_id) = gchat_config.project_id {
        tracing::info!(channel = "googlechat", project_id = %project_id, "project configured");
    }

    tracing::info!(
        channel = "googlechat",
        port = port,
        "adapter started (webhook mode)"
    );

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
    });

    let app = Router::new()
        .route("/", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!(channel = "googlechat", addr = %addr, "listening");

    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Google Chat bot.
#[allow(dead_code)]
pub struct GoogleChatAdapter {
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl GoogleChatAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for GoogleChatAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "googlechat".to_string(),
            name: "Google Chat".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(4096),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "googlechat", "GoogleChatAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "googlechat", "GoogleChatAdapter stopped");
        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => ChannelStatus::Connected,
            STATUS_ERROR => ChannelStatus::Error("adapter error".to_string()),
            _ => ChannelStatus::Disconnected,
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl Outbound for GoogleChatAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "googlechat",
            content_len = envelope.content.len(),
            "GoogleChatAdapter::send (placeholder)"
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for GoogleChatAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter error".to_string()),
            _ => HealthStatus::Unhealthy("disconnected".to_string()),
        }
    }
}
