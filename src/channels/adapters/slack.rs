use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::channels::reactions;
use crate::config::bot::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

/// Run the Slack bot adapter using Socket Mode.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let slack_config = config
        .slack
        .as_ref()
        .ok_or("missing [slack] section in config")?;

    let app_token = resolve_secret(
        slack_config.app_token.as_deref(),
        slack_config.app_token_env.as_deref(),
        "Slack app token",
    )
    .map_err(|e| format!("{}", e))?;
    let bot_token = resolve_secret(
        slack_config.bot_token.as_deref(),
        slack_config.bot_token_env.as_deref(),
        "Slack bot token",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = slack_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true).with_channel("slack"));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "slack",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "slack", "adapter started");

    loop {
        match run_socket_mode(&app_token, &bot_token, agent_session.clone(), &allowlist).await {
            Ok(()) => break,
            Err(e) => {
                tracing::warn!(channel = "slack", error = %e, "connection error, reconnecting");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    Ok(())
}

/// Send a typing indicator to a Slack channel.
///
/// Note: Slack's Socket Mode does not expose a typing API for bots. The
/// `chat.postMessage` API only sends messages, not ephemeral typing indicators.
/// Alternative approach: post a temporary "Thinking..." message and update it
/// with the real response via `chat.update`. This is left as a future enhancement
/// since it requires tracking the temporary message ts.
async fn send_typing(_bot_token: &str, _channel: &str) {
    // Slack Socket Mode has no typing API for bots — intentional no-op.
}

async fn run_socket_mode(
    app_token: &str,
    bot_token: &str,
    agent_session: Arc<AgentSession>,
    allowlist: &BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Open a WebSocket connection via apps.connections.open
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    if !body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("apps.connections.open failed: {}", body).into());
    }

    let ws_url = body
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("no WebSocket URL returned")?;

    // Step 2: Connect WebSocket
    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    tracing::info!(channel = "slack", "socket mode connected");

    // Step 3: Handle events
    while let Some(msg) = read.next().await {
        let msg = msg?;
        let WsMsg::Text(text) = msg else { continue };

        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Acknowledge the envelope
        if let Some(envelope_id) = payload.get("envelope_id").and_then(|v| v.as_str()) {
            let ack = serde_json::json!({"envelope_id": envelope_id});
            write.send(WsMsg::Text(ack.to_string().into())).await.ok();
        }

        // Check event type
        let event_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if event_type != "events_api" {
            continue;
        }

        // Extract message event
        let event = match payload.get("payload").and_then(|p| p.get("event")) {
            Some(e) => e,
            None => continue,
        };

        let msg_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "message" {
            continue;
        }

        // Skip bot messages (prevent echo)
        if event.get("bot_id").is_some() {
            continue;
        }

        let text = event
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let channel = event
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let user_id = event.get("user").and_then(|v| v.as_str()).unwrap_or("");

        let ts = event
            .get("ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() || channel.is_empty() {
            continue;
        }

        // Allowlist check
        if !allowlist.is_allowed(Some(user_id), Some(&channel)) {
            continue;
        }

        // Process in background
        let session = agent_session.clone();
        let bot_token = bot_token.to_string();
        tokio::spawn(async move {
            // React with eyes to indicate processing
            reactions::slack_react(&bot_token, &channel, &ts, "eyes").await;

            // Send typing indicator
            send_typing(&bot_token, &channel).await;

            let delivery = DeliveryContext {
                channel: "slack".into(),
                to: Some(format!("channel:{}", channel)),
                thread_id: Some(ts.clone()),
                ..Default::default()
            };
            let envelope = MessageEnvelope::channel(channel.clone(), text, delivery);

            match session.handle_message(envelope).await {
                Ok(reply) => {
                    // Split long replies into chunks
                    let chunks = formatter::chunk_slack(&reply.content);
                    let client = reqwest::Client::new();
                    for chunk in chunks {
                        let _ = client
                            .post("https://slack.com/api/chat.postMessage")
                            .bearer_auth(&bot_token)
                            .json(&serde_json::json!({
                                "channel": channel,
                                "text": chunk,
                            }))
                            .send()
                            .await;
                    }
                    // React with checkmark on success
                    reactions::slack_react(&bot_token, &channel, &ts, "white_check_mark").await;
                }
                Err(e) => {
                    tracing::error!(channel = "slack", error = %e, "message handler error");
                }
            }
        });
    }

    Ok(())
}
