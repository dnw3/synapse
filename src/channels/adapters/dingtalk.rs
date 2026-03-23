use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

use tracing;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};

type HmacSha256 = Hmac<Sha256>;

/// Shared state for the axum callback server.
#[allow(dead_code)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
    app_secret: String,
    robot_code: Option<String>,
}

/// DingTalk callback event payload (subset of fields we care about).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CallbackPayload {
    /// Message headers with signature info.
    #[serde(default)]
    headers: Option<CallbackHeaders>,
    /// The message content.
    #[serde(default)]
    text: Option<TextContent>,
    /// Sender information.
    #[serde(default)]
    sender_id: Option<String>,
    /// Conversation ID (group or 1:1).
    #[serde(default)]
    conversation_id: Option<String>,
    /// Conversation type: "1" = 1:1, "2" = group.
    #[serde(default)]
    conversation_type: Option<String>,
    /// The incoming webhook URL to reply to.
    #[serde(default)]
    session_webhook: Option<String>,
    /// Message type (e.g. "text").
    #[serde(default)]
    msgtype: Option<String>,
    /// Message ID.
    #[serde(default)]
    msg_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CallbackHeaders {
    #[serde(default)]
    sign: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct TextContent {
    content: Option<String>,
}

/// Verify the DingTalk callback signature.
///
/// DingTalk signs callbacks with HMAC-SHA256:
///   sign = Base64(HmacSHA256(timestamp + "\n" + app_secret))
fn verify_signature(timestamp: &str, app_secret: &str, expected_sign: &str) -> bool {
    let string_to_sign = format!("{}\n{}", timestamp, app_secret);

    let Ok(mut mac) = HmacSha256::new_from_slice(app_secret.as_bytes()) else {
        return false;
    };
    mac.update(string_to_sign.as_bytes());
    let result = mac.finalize();
    let computed = base64_encode(&result.into_bytes());

    computed == expected_sign
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

/// Handle incoming DingTalk callback POST.
async fn handle_callback(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<CallbackPayload>,
) -> StatusCode {
    // Verify signature if headers are present
    if let Some(ref headers) = payload.headers {
        if let (Some(ref sign), Some(ref timestamp)) = (&headers.sign, &headers.timestamp) {
            if !verify_signature(timestamp, &state.app_secret, sign) {
                tracing::error!(channel = "dingtalk", "signature verification failed");
                return StatusCode::UNAUTHORIZED;
            }
        }
    }

    // Extract text content
    let text = match payload.text.as_ref().and_then(|t| t.content.as_deref()) {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return StatusCode::OK, // Ignore non-text or empty messages
    };

    let sender_id = payload.sender_id.clone().unwrap_or_default();
    let conversation_id = payload.conversation_id.clone().unwrap_or_default();

    // Allowlist check
    if !state.allowlist.is_allowed(
        if sender_id.is_empty() {
            None
        } else {
            Some(&sender_id)
        },
        if conversation_id.is_empty() {
            None
        } else {
            Some(&conversation_id)
        },
    ) {
        return StatusCode::OK; // Silently ignore unauthorized
    }

    // Use session webhook for replies if available
    let webhook_url = match payload.session_webhook.clone() {
        Some(url) if !url.is_empty() => url,
        _ => {
            tracing::warn!(
                channel = "dingtalk",
                "no session_webhook in payload, cannot reply"
            );
            return StatusCode::OK;
        }
    };

    // Determine session key from conversation or sender
    let session_key = if !conversation_id.is_empty() {
        format!("dingtalk:{}", conversation_id)
    } else {
        format!("dingtalk:{}", sender_id)
    };

    let is_dm = payload.conversation_type.as_deref() == Some("1");

    // Process in background so we respond to DingTalk quickly
    let session = state.agent_session.clone();
    tokio::spawn(async move {
        let channel_info = ChannelInfo {
            platform: "dingtalk".into(),
            native_channel_id: Some(conversation_id.clone()),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(sender_id.clone()),
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
        match session.handle_message(msg, RunContext::default()).await {
            Ok(reply) => {
                let client = reqwest::Client::new();
                let chunks = formatter::format_for_channel(&reply.content, "dingtalk", 20000);
                for chunk in chunks {
                    let body = serde_json::json!({
                        "msgtype": "text",
                        "text": {
                            "content": chunk,
                        }
                    });

                    if let Err(e) = client.post(&webhook_url).json(&body).send().await {
                        tracing::error!(channel = "dingtalk", error = %e, "failed to send reply");
                    }
                }
            }
            Err(e) => {
                tracing::error!(channel = "dingtalk", error = %e, "handler error");
                // Try to send error message back
                let client = reqwest::Client::new();
                let body = serde_json::json!({
                    "msgtype": "text",
                    "text": {
                        "content": format!("Error: {}", e),
                    }
                });
                let _ = client.post(&webhook_url).json(&body).send().await;
            }
        }
    });

    StatusCode::OK
}

/// Run the DingTalk bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dt_configs: Vec<crate::config::DingTalkBotConfig> = config.channel_configs("dingtalk");
    let dt_config = dt_configs
        .first()
        .ok_or("missing [[channels.dingtalk]] section in config")?;

    let app_secret = resolve_secret(
        dt_config.app_secret.as_deref(),
        dt_config.app_secret_env.as_deref(),
        "DingTalk app secret",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = dt_config.allowlist.clone();
    let agent_session =
        Arc::new(AgentSession::new(model, config_arc, true).with_channel("dingtalk"));

    let port = dt_config.callback_port.unwrap_or(8075);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "dingtalk",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
        app_secret,
        robot_code: dt_config.robot_code.clone(),
    });

    let app = Router::new()
        .route("/callback", post(handle_callback))
        .with_state(state);

    tracing::info!(
        channel = "dingtalk",
        app_key = %dt_config.app_key,
        port = port,
        "adapter started"
    );

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the DingTalk bot.
#[allow(dead_code)]
pub struct DingTalkAdapter {
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl DingTalkAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for DingTalkAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "dingtalk".to_string(),
            name: "DingTalk".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(20000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "dingtalk", "DingTalkAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "dingtalk", "DingTalkAdapter stopped");
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
impl Outbound for DingTalkAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "dingtalk",
            content_len = envelope.content.len(),
            "DingTalkAdapter::send (placeholder)"
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for DingTalkAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter error".to_string()),
            _ => HealthStatus::Unhealthy("disconnected".to_string()),
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
        let secret = "test_secret";
        let timestamp = "1234567890";
        let string_to_sign = format!("{}\n{}", timestamp, secret);

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        let sign = base64_encode(&result.into_bytes());

        assert!(verify_signature(timestamp, secret, &sign));
        assert!(!verify_signature(timestamp, secret, "wrong_sign"));
    }
}
