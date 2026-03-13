use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::MessageEnvelope;

/// Run the iMessage bot adapter using the BlueBubbles REST API bridge.
///
/// Polls `GET /api/v1/message?limit=10&offset=0&after=<timestamp>&password=<pw>`
/// for incoming messages and replies via `POST /api/v1/message/text`.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let imessage_config = config
        .imessage
        .as_ref()
        .ok_or("missing [imessage] section in config")?;

    let password = resolve_secret(
        imessage_config.password.as_deref(),
        imessage_config.password_env.as_deref(),
        "iMessage password",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = imessage_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let api_url = imessage_config.api_url.trim_end_matches('/').to_string();

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "imessage",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "imessage", "adapter started");
    tracing::info!(channel = "imessage", api_url = %api_url, "polling started");

    let client = reqwest::Client::new();

    // Track the last poll timestamp in milliseconds (Unix epoch).
    // BlueBubbles uses millisecond timestamps for the `after` parameter.
    let mut last_timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    loop {
        let poll_url = format!(
            "{}/api/v1/message?limit=10&offset=0&after={}&password={}",
            api_url,
            last_timestamp_ms,
            urlencoding::encode(&password)
        );

        let resp = match client.get(&poll_url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(channel = "imessage", error = %e, "polling error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(channel = "imessage", error = %e, "response parse error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // BlueBubbles wraps results in { "status": "ok", "data": [...] }
        let messages = match body.get("data").and_then(|d| d.as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        // Advance the timestamp so the next poll only returns newer messages.
        // Use the current wall-clock time to avoid re-processing the same batch.
        last_timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        for message in messages {
            // Only process incoming text messages (not sent by the bot itself)
            let is_from_me = message
                .get("isFromMe")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if is_from_me {
                continue;
            }

            // Extract the message text
            let text = match message.get("text").and_then(|v| v.as_str()) {
                Some(t) if !t.is_empty() => t.to_string(),
                _ => continue,
            };

            // The chat GUID identifies the conversation to reply to
            let chat_guid = match message
                .get("chats")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|chat| chat.get("guid"))
                .and_then(|v| v.as_str())
            {
                Some(g) => g.to_string(),
                None => continue,
            };

            // The sender handle is used as the session/user identifier
            let sender = match message
                .get("handle")
                .and_then(|h| h.get("address"))
                .and_then(|v| v.as_str())
            {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => chat_guid.clone(),
            };

            // Allowlist check: sender is the user, chat_guid acts as the channel
            if !allowlist.is_allowed(Some(&sender), Some(&chat_guid)) {
                continue;
            }

            // Process message in background
            let session = agent_session.clone();
            let http = client.clone();
            let send_url = format!("{}/api/v1/message/text", api_url);
            let pw = password.clone();
            let reply_chat_guid = chat_guid.clone();
            let session_key = sender.clone();

            tokio::spawn(async move {
                let envelope = MessageEnvelope::channel(
                    session_key.clone(),
                    text.clone(),
                    DeliveryContext {
                        channel: "imessage".into(),
                        to: Some(format!("user:{}", session_key)),
                        account_id: None,
                        thread_id: None,
                        meta: None,
                    },
                );
                match session.handle_message(envelope).await {
                    Ok(reply) => {
                        let chunks = formatter::chunk_imessage(&reply.content);
                        for chunk in chunks {
                            let body = serde_json::json!({
                                "chatGuid": reply_chat_guid,
                                "message": chunk,
                                "password": pw,
                            });
                            if let Err(e) = http.post(&send_url).json(&body).send().await {
                                tracing::error!(channel = "imessage", error = %e, "send error");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(channel = "imessage", error = %e, "handler error");
                    }
                }
            });
        }
    }
}
