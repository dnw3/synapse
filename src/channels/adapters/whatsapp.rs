use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::MessageEnvelope;

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
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wa_config = config
        .whatsapp
        .as_ref()
        .ok_or("missing [whatsapp] section in config")?;

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
) -> Result<(), Box<dyn std::error::Error>> {
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

        // Process the message in a background task so we don't block the event loop.
        let session = agent_session.clone();
        let bridge = bridge_url.to_string();
        let api_key_owned = api_key.map(|k| k.to_string());
        tokio::spawn(async move {
            let envelope = MessageEnvelope::channel(
                chat_id.clone(),
                body.clone(),
                DeliveryContext {
                    channel: "whatsapp".into(),
                    to: Some(format!("user:{}", chat_id)),
                    account_id: None,
                    thread_id: None,
                    meta: None,
                },
            );
            match session.handle_message(envelope).await {
                Ok(reply) => {
                    let chunks = formatter::chunk_whatsapp(&reply.content);
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
