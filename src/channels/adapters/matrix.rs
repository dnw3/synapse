use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};

/// Run the Matrix bot adapter using the Client-Server REST API (long-polling sync).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mx_config = config
        .matrix
        .first()
        .ok_or("missing [[matrix]] section in config")?;

    let password = resolve_secret(
        mx_config.password.as_deref(),
        mx_config.password_env.as_deref(),
        "Matrix password",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = mx_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "matrix",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "matrix", "adapter started");

    let client = reqwest::Client::new();
    let homeserver = mx_config.homeserver_url.trim_end_matches('/').to_string();
    let user_id = mx_config.user_id.clone();

    // Login and obtain an access token
    let access_token = login(&client, &homeserver, &user_id, &password).await?;

    tracing::info!(channel = "matrix", user_id = %user_id, "logged in");

    // Main sync loop
    let mut since: Option<String> = None;

    loop {
        let result = sync_once(&client, &homeserver, &access_token, since.as_deref()).await;

        match result {
            Ok((next_batch, events)) => {
                since = Some(next_batch);

                for event in events {
                    let room_id = match event.room_id {
                        Some(ref r) => r.clone(),
                        None => continue,
                    };
                    let sender = match event.sender {
                        Some(ref s) => s.clone(),
                        None => continue,
                    };
                    let text = match event.body {
                        Some(ref t) => t.clone(),
                        None => continue,
                    };

                    // Skip our own messages
                    if sender == user_id {
                        continue;
                    }

                    // Allowlist check
                    if !allowlist.is_allowed(Some(&sender), Some(&room_id)) {
                        continue;
                    }

                    // Process in background
                    let session = agent_session.clone();
                    let http = client.clone();
                    let hs = homeserver.clone();
                    let token = access_token.clone();
                    let rid = room_id.clone();
                    let sender_clone = sender.clone();
                    tokio::spawn(async move {
                        let channel_info = ChannelInfo {
                            platform: "matrix".into(),
                            native_channel_id: Some(rid.clone()),
                            ..Default::default()
                        };
                        let sender_info = SenderInfo {
                            id: Some(sender_clone),
                            ..Default::default()
                        };
                        let chat_info = ChatInfo {
                            chat_type: "group".into(),
                            ..Default::default()
                        };
                        let mut msg = InboundMessage::channel(
                            rid.clone(),
                            text.clone(),
                            channel_info,
                            sender_info,
                            chat_info,
                        );
                        msg.finalize();
                        match session.handle_inbound(msg).await {
                            Ok(reply) => {
                                let chunks =
                                    formatter::format_for_channel(&reply.content, "matrix", 60000);
                                for chunk in chunks {
                                    send_message(&http, &hs, &token, &rid, &chunk).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!(channel = "matrix", error = %e, "handler error");
                            }
                        }
                    });
                }
            }
            Err(e) => {
                tracing::warn!(channel = "matrix", error = %e, "sync error, retrying");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// A parsed m.room.message event extracted from a sync response.
struct RoomEvent {
    room_id: Option<String>,
    sender: Option<String>,
    body: Option<String>,
}

/// Login via `POST /_matrix/client/v3/login` using m.login.password.
///
/// Returns the access token on success.
async fn login(
    client: &reqwest::Client,
    homeserver: &str,
    user_id: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/_matrix/client/v3/login", homeserver);
    let body = serde_json::json!({
        "type": "m.login.password",
        "identifier": {
            "type": "m.id.user",
            "user": user_id,
        },
        "password": password,
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Matrix login request failed: {}", e))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Matrix login response parse error: {}", e))?;

    if !status.is_success() {
        let err = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("Matrix login failed ({}): {}", status, err).into());
    }

    json.get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Matrix login response missing access_token".into())
}

/// Perform one long-poll sync request.
///
/// Uses a 30-second timeout for the long-poll. Returns the next `since` token
/// and a list of extracted room message events.
async fn sync_once(
    client: &reqwest::Client,
    homeserver: &str,
    access_token: &str,
    since: Option<&str>,
) -> Result<(String, Vec<RoomEvent>), Box<dyn std::error::Error>> {
    let mut url = format!(
        "{}/_matrix/client/v3/sync?timeout=30000&filter={}",
        homeserver,
        urlencoding::encode(r#"{"room":{"timeline":{"limit":50}}}"#)
    );
    if let Some(s) = since {
        url.push_str(&format!("&since={}", urlencoding::encode(s)));
    }

    let resp = client
        .get(&url)
        .bearer_auth(access_token)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("Matrix sync request failed: {}", e))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Matrix sync response parse error: {}", e))?;

    if !status.is_success() {
        let err = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("Matrix sync failed ({}): {}", status, err).into());
    }

    let next_batch = json
        .get("next_batch")
        .and_then(|v| v.as_str())
        .ok_or("Matrix sync response missing next_batch")?
        .to_string();

    let mut events = Vec::new();

    // Walk rooms.join.<room_id>.timeline.events
    if let Some(rooms) = json.get("rooms").and_then(|r| r.get("join")) {
        if let Some(joined) = rooms.as_object() {
            for (room_id, room_data) in joined {
                let timeline_events = room_data
                    .get("timeline")
                    .and_then(|t| t.get("events"))
                    .and_then(|e| e.as_array());

                if let Some(ev_list) = timeline_events {
                    for ev in ev_list {
                        // Only handle m.room.message events with msgtype m.text
                        let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        if ev_type != "m.room.message" {
                            continue;
                        }

                        let msgtype = ev
                            .get("content")
                            .and_then(|c| c.get("msgtype"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if msgtype != "m.text" {
                            continue;
                        }

                        let body = ev
                            .get("content")
                            .and_then(|c| c.get("body"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let sender = ev
                            .get("sender")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        events.push(RoomEvent {
                            room_id: Some(room_id.clone()),
                            sender,
                            body,
                        });
                    }
                }
            }
        }
    }

    Ok((next_batch, events))
}

/// Send a text message to a Matrix room via
/// `PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}`.
async fn send_message(
    client: &reqwest::Client,
    homeserver: &str,
    access_token: &str,
    room_id: &str,
    text: &str,
) {
    let txn_id = uuid::Uuid::new_v4().to_string();
    let encoded_room = urlencoding::encode(room_id);
    let url = format!(
        "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
        homeserver, encoded_room, txn_id
    );

    let body = serde_json::json!({
        "msgtype": "m.text",
        "body": text,
    });

    if let Err(e) = client
        .put(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await
    {
        tracing::error!(channel = "matrix", error = %e, "send error");
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Matrix bot.
#[allow(dead_code)]
pub struct MatrixAdapter {
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl MatrixAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for MatrixAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "matrix".to_string(),
            name: "Matrix".to_string(),
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
        tracing::info!(channel = "matrix", "MatrixAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "matrix", "MatrixAdapter stopped");
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
impl Outbound for MatrixAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "matrix",
            content_len = envelope.content.len(),
            "MatrixAdapter::send (placeholder)"
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for MatrixAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter error".to_string()),
            _ => HealthStatus::Unhealthy("disconnected".to_string()),
        }
    }
}
