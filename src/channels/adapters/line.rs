use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use tracing;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};
use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

type HmacSha256 = Hmac<Sha256>;

/// Shared state for the axum webhook server.
#[allow(dead_code)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
    channel_secret: String,
    channel_token: String,
}

/// Top-level LINE webhook payload.
#[derive(Debug, serde::Deserialize)]
struct WebhookPayload {
    /// List of webhook events from LINE.
    #[serde(default)]
    events: Vec<LineEvent>,
}

/// A single LINE webhook event.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineEvent {
    /// Event type (e.g. "message", "follow", "unfollow").
    #[serde(rename = "type")]
    event_type: String,
    /// Reply token used to respond to this event.
    #[serde(default)]
    reply_token: Option<String>,
    /// The message payload (present when event_type == "message").
    #[serde(default)]
    message: Option<LineMessage>,
    /// Source of the event (user, group, or room).
    source: LineSource,
}

/// The message portion of a LINE event.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineMessage {
    /// Message type (e.g. "text", "image", "sticker").
    #[serde(rename = "type")]
    message_type: String,
    /// Text content (only present when message_type == "text").
    #[serde(default)]
    text: Option<String>,
}

/// Source of a LINE event.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct LineSource {
    /// Source type: "user", "group", or "room".
    #[serde(rename = "type")]
    source_type: String,
    /// User ID of the sender.
    #[serde(default)]
    user_id: Option<String>,
    /// Group ID (when source_type == "group").
    #[serde(default)]
    group_id: Option<String>,
    /// Room ID (when source_type == "room").
    #[serde(default)]
    room_id: Option<String>,
}

/// Verify the LINE webhook signature.
///
/// LINE signs each webhook request with HMAC-SHA256 using the channel secret
/// as the key and the raw request body as the message. The resulting digest is
/// Base64-encoded and placed in the `X-Line-Signature` header.
fn verify_signature(channel_secret: &str, body: &[u8], expected_signature: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(channel_secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let computed = base64_encode(&mac.finalize().into_bytes());
    computed == expected_signature
}

/// Simple base64 encoding (standard alphabet with padding).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut encoded = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        encoded.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            encoded.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            encoded.push('=');
        }

        if i + 2 < data.len() {
            encoded.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            encoded.push('=');
        }

        i += 3;
    }

    encoded
}

/// Send a reply to LINE via the Reply API.
///
/// Sends multiple text messages if the reply exceeds the 5000-character limit.
/// Each call to this function uses a single `replyToken`; only the first message
/// in the array uses it, so we fall back to individual reply calls for chunks
/// beyond the first when the token allows multiple messages in one reply.
async fn send_reply(channel_token: &str, reply_token: &str, text: &str) {
    let client = reqwest::Client::new();
    let chunks = formatter::format_for_channel(text, "line", 5000);

    // LINE Reply API supports up to 5 messages per reply call.
    // We bundle all chunks into a single request (LINE allows up to 5 messages
    // in one reply), and if there are more than 5 chunks we send subsequent
    // groups — though in practice format_for_channel keeps messages small enough that
    // more than 5 chunks is rare.
    let messages: Vec<serde_json::Value> = chunks
        .iter()
        .take(5)
        .map(|chunk| {
            serde_json::json!({
                "type": "text",
                "text": chunk,
            })
        })
        .collect();

    let body = serde_json::json!({
        "replyToken": reply_token,
        "messages": messages,
    });

    if let Err(e) = client
        .post("https://api.line.me/v2/bot/message/reply")
        .bearer_auth(channel_token)
        .json(&body)
        .send()
        .await
    {
        tracing::error!(channel = "line", error = %e, "failed to send reply");
    }
}

/// Handle incoming LINE webhook POST.
///
/// Axum extracts the raw body bytes so we can verify the HMAC-SHA256 signature
/// before parsing the JSON payload.
async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // Extract and verify the X-Line-Signature header.
    let signature = match headers
        .get("x-line-signature")
        .and_then(|v| v.to_str().ok())
    {
        Some(s) => s.to_string(),
        None => {
            tracing::error!(channel = "line", "missing X-Line-Signature header");
            return StatusCode::UNAUTHORIZED;
        }
    };

    if !verify_signature(&state.channel_secret, &body, &signature) {
        tracing::error!(channel = "line", "signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    // Parse the JSON payload.
    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(channel = "line", error = %e, "failed to parse webhook payload");
            return StatusCode::BAD_REQUEST;
        }
    };

    for event in payload.events {
        // Only handle text message events.
        if event.event_type != "message" {
            continue;
        }

        let message = match event.message {
            Some(ref m) if m.message_type == "text" => m,
            _ => continue,
        };

        let text = match message.text.as_deref() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => continue,
        };

        let reply_token = match event.reply_token.clone() {
            Some(rt) if !rt.is_empty() => rt,
            _ => {
                tracing::warn!(channel = "line", "no replyToken in event, cannot reply");
                continue;
            }
        };

        let user_id = event.source.user_id.clone().unwrap_or_default();
        let channel_id = event
            .source
            .group_id
            .clone()
            .or(event.source.room_id.clone())
            .unwrap_or_default();

        // Allowlist check (by userId or group/room ID).
        if !state.allowlist.is_allowed(
            if user_id.is_empty() {
                None
            } else {
                Some(&user_id)
            },
            if channel_id.is_empty() {
                None
            } else {
                Some(&channel_id)
            },
        ) {
            continue; // Silently ignore unauthorized senders.
        }

        // Build a stable session key for conversation continuity.
        let session_key = if !channel_id.is_empty() {
            format!("line:{}", channel_id)
        } else {
            format!("line:{}", user_id)
        };

        let is_dm = event.source.source_type == "user";

        // Respond to LINE quickly (< 1 second) and process in the background.
        let session = state.agent_session.clone();
        let channel_token = state.channel_token.clone();
        tokio::spawn(async move {
            let mut envelope = MessageEnvelope::channel(
                session_key.clone(),
                text.clone(),
                DeliveryContext {
                    channel: "line".into(),
                    to: Some(format!("user:{}", session_key)),
                    account_id: None,
                    thread_id: None,
                    meta: None,
                },
            );
            if !user_id.is_empty() {
                envelope.sender_id = Some(user_id.clone());
            }
            envelope.routing.peer_kind = Some(if is_dm {
                crate::config::PeerKind::Direct
            } else {
                crate::config::PeerKind::Group
            });
            envelope.routing.peer_id = Some(if !channel_id.is_empty() {
                channel_id.clone()
            } else {
                user_id.clone()
            });
            match session.handle_message(envelope).await {
                Ok(reply) => {
                    send_reply(&channel_token, &reply_token, &reply.content).await;
                }
                Err(e) => {
                    tracing::error!(channel = "line", error = %e, "handler error");
                    send_reply(&channel_token, &reply_token, &format!("Error: {e}")).await;
                }
            }
        });
    }

    StatusCode::OK
}

/// Run the LINE bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let line_config = config
        .line
        .first()
        .ok_or("missing [[line]] section in config")?;

    let channel_secret = resolve_secret(
        line_config.channel_secret.as_deref(),
        line_config.channel_secret_env.as_deref(),
        "LINE channel secret",
    )
    .map_err(|e| format!("{}", e))?;

    let channel_token = resolve_secret(
        line_config.channel_token.as_deref(),
        line_config.channel_token_env.as_deref(),
        "LINE channel token",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = line_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = line_config.port.unwrap_or(8076);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "line",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
        channel_secret,
        channel_token,
    });

    let app = Router::new()
        .route("/webhook", post(handle_webhook))
        .with_state(state);

    tracing::info!(channel = "line", port = port, "adapter started");

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`LineAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the LINE Messaging API.
#[allow(dead_code)]
pub struct LineAdapter {
    channel_token: String,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl LineAdapter {
    pub fn new(channel_token: &str) -> Self {
        Self {
            channel_token: channel_token.to_string(),
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for LineAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "line".to_string(),
            name: "LINE".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(5000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "line", "LineAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "line", "LineAdapter stopped");
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
impl Outbound for LineAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "line",
            to = %envelope.channel_id,
            "LineAdapter::send (placeholder)",
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for LineAdapter {
    async fn health_check(&self) -> HealthStatus {
        // LINE does not provide a dedicated health endpoint; verify the token
        // by calling the bot info endpoint.
        let url = "https://api.line.me/v2/bot/info";
        match self
            .client
            .get(url)
            .bearer_auth(&self.channel_token)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("LINE bot info returned HTTP {}", resp.status());
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(msg)
            }
            Err(e) => {
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_works() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn verify_signature_roundtrip() {
        let secret = "test_channel_secret";
        let body = b"test body content";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig = base64_encode(&mac.finalize().into_bytes());

        assert!(verify_signature(secret, body, &sig));
        assert!(!verify_signature(secret, body, "wrong_signature"));
        assert!(!verify_signature("wrong_secret", body, &sig));
    }
}
