use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use tracing;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

/// Run the WhatsApp bot adapter using a Baileys-compatible REST/WebSocket bridge.
///
/// The adapter connects to a whatsapp-web.js bridge API (e.g. wweb.js or Baileys REST bridge)
/// that exposes:
/// - `GET  <bridge_url>/ws`            — WebSocket endpoint for incoming message events
/// - `POST <bridge_url>/send`          — Send a text message (`{ "to": "...", "text": "..." }`)
///
/// The bridge is responsible for maintaining the WhatsApp Web session and forwarding
/// messages over the WebSocket in the following JSON schema:
///
/// ```json
/// {
///   "type": "message",
///   "from": "<phone-number>@s.whatsapp.net",
///   "chatId": "<chat-id>@s.whatsapp.net",
///   "body": "Hello!"
/// }
/// ```
pub async fn run(config: &SynapseConfig, model_override: Option<&str>) -> crate::error::Result<()> {
    let wa_configs: Vec<crate::config::WhatsAppBotConfig> = config.channel_configs("whatsapp");
    let wa_config = wa_configs
        .first()
        .ok_or("missing [[channels.whatsapp]] section in config")?;

    // Optionally resolve an API key (used as Bearer token for bridge).
    let api_key: Option<String> = resolve_secret(
        wa_config.access_token.as_deref(),
        wa_config.api_key_env.as_deref(),
        "WhatsApp API key",
    )
    .ok();

    let bridge_url = wa_config
        .bridge_url
        .as_deref()
        .unwrap_or("http://localhost:29318")
        .trim_end_matches('/')
        .to_string();
    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = wa_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "whatsapp",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "whatsapp", "adapter started");

    loop {
        match run_ws_loop(
            &bridge_url,
            api_key.as_deref(),
            agent_session.clone(),
            &allowlist,
        )
        .await
        {
            Ok(()) => break,
            Err(e) => {
                tracing::warn!(channel = "whatsapp", error = %e, "bridge connection error, reconnecting in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    Ok(())
}

/// Connect to the bridge WebSocket and handle incoming message events.
async fn run_ws_loop(
    bridge_url: &str,
    api_key: Option<&str>,
    agent_session: Arc<AgentSession>,
    allowlist: &crate::config::BotAllowlist,
) -> crate::error::Result<()> {
    // Build the WebSocket URL — replace http(s) scheme with ws(s).
    let ws_url = if bridge_url.starts_with("https://") {
        format!("wss://{}/ws", &bridge_url["https://".len()..])
    } else if bridge_url.starts_with("http://") {
        format!("ws://{}/ws", &bridge_url["http://".len()..])
    } else {
        // Assume it's already a ws(s) URL or a bare host.
        format!("{}/ws", bridge_url)
    };

    // Build WebSocket request, optionally adding Authorization header.
    let ws_request = {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = ws_url.as_str().into_client_request()?;
        if let Some(key) = api_key {
            req.headers_mut()
                .insert("Authorization", format!("Bearer {}", key).parse()?);
        }
        req
    };

    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_request).await?;
    let (mut write, mut read) = ws_stream.split();

    tracing::info!(channel = "whatsapp", "bridge websocket connected");

    while let Some(msg) = read.next().await {
        let msg = msg?;

        // Send pong for ping frames to keep the connection alive.
        if let WsMsg::Ping(data) = &msg {
            write.send(WsMsg::Pong(data.clone())).await.ok();
            continue;
        }

        let WsMsg::Text(text) = msg else { continue };

        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only handle incoming message events.
        let event_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if event_type != "message" {
            continue;
        }

        let body = payload
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // `chatId` is the conversation key (group or 1:1 chat JID).
        let chat_id = payload
            .get("chatId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // `from` is the sender JID (phone number).
        let from = payload
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Skip bot's own messages if the bridge echoes them back.
        if payload
            .get("fromMe")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }

        if body.is_empty() || chat_id.is_empty() {
            continue;
        }

        // Allowlist check — user is the JID sender, channel is the chat JID.
        let user_id_opt = if from.is_empty() {
            None
        } else {
            Some(from.as_str())
        };
        let channel_opt = if chat_id.is_empty() {
            None
        } else {
            Some(chat_id.as_str())
        };
        if !allowlist.is_allowed(user_id_opt, channel_opt) {
            continue;
        }

        // WhatsApp group JIDs end with @g.us, DM JIDs with @s.whatsapp.net
        let is_group = chat_id.contains("@g.us");

        // Process the message in a background task so we don't block the event loop.
        let session = agent_session.clone();
        let bridge = bridge_url.to_string();
        let api_key_owned = api_key.map(|k| k.to_string());
        tokio::spawn(async move {
            let channel_info = ChannelInfo {
                platform: "whatsapp".into(),
                native_channel_id: Some(chat_id.clone()),
                ..Default::default()
            };
            let sender_info = SenderInfo {
                id: if from.is_empty() {
                    None
                } else {
                    Some(from.clone())
                },
                ..Default::default()
            };
            let chat_info = ChatInfo {
                chat_type: if is_group { "group" } else { "direct" }.to_string(),
                ..Default::default()
            };
            let mut msg = InboundMessage::channel(
                chat_id.clone(),
                body.clone(),
                channel_info,
                sender_info,
                chat_info,
            );
            msg.finalize();
            match session.handle_message(msg, RunContext::default()).await {
                Ok(reply) => {
                    let chunks = formatter::format_for_channel(&reply.content, "whatsapp", 2000);
                    let http = reqwest::Client::new();
                    for chunk in chunks {
                        let send_url = format!("{}/send", bridge);
                        let mut req = http.post(&send_url).json(&serde_json::json!({
                            "to": chat_id,
                            "text": chunk,
                        }));
                        if let Some(ref key) = api_key_owned {
                            req = req.bearer_auth(key);
                        }
                        if let Err(e) = req.send().await {
                            tracing::error!(channel = "whatsapp", error = %e, "send error");
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(channel = "whatsapp", error = %e, "handler error");
                }
            }
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`WhatsAppAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the WhatsApp bridge.
#[allow(dead_code)]
pub struct WhatsAppAdapter {
    bridge_url: String,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl WhatsAppAdapter {
    pub fn new(bridge_url: &str) -> Self {
        Self {
            bridge_url: bridge_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for WhatsAppAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "whatsapp".to_string(),
            name: "WhatsApp".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(65536),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "whatsapp", "WhatsAppAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "whatsapp", "WhatsAppAdapter stopped");
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
impl Outbound for WhatsAppAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "whatsapp",
            to = %envelope.channel_id,
            "WhatsAppAdapter::send (placeholder)",
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for WhatsAppAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/health", self.bridge_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("bridge health returned HTTP {}", resp.status());
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
