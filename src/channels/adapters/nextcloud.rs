use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use tokio::time::Duration;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{NextcloudBotConfig, SynapseConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

/// Run the Nextcloud Talk bot adapter using REST long-polling.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let nc_configs: Vec<crate::config::NextcloudBotConfig> = config.channel_configs("nextcloud");
    let nc_config = nc_configs
        .first()
        .ok_or("missing [[channels.nextcloud]] section in config")?;

    let password = resolve_secret(
        nc_config.password.as_deref(),
        nc_config.password_env.as_deref(),
        "Nextcloud password",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let client = Client::new();
    let base_url = nc_config.url.trim_end_matches('/');

    tracing::info!(channel = "nextcloud", url = %base_url, "adapter started (polling mode)");

    let poll_interval = Duration::from_secs(nc_config.poll_interval_secs.unwrap_or(3));
    let mut last_known_id: u64 = 0;

    loop {
        // Poll for new messages via Nextcloud Talk API
        for room_token in &nc_config.rooms {
            let url = format!(
                "{}/ocs/v2.php/apps/spreed/api/v1/chat/{}",
                base_url, room_token
            );

            let resp = client
                .get(&url)
                .basic_auth(&nc_config.username, Some(&password))
                .header("OCS-APIRequest", "true")
                .header("Accept", "application/json")
                .query(&[
                    ("lookIntoFuture", "0"),
                    ("limit", "20"),
                    ("setReadMarker", "0"),
                ])
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(body) = r.json::<serde_json::Value>().await {
                        if let Some(messages) = body
                            .get("ocs")
                            .and_then(|o| o.get("data"))
                            .and_then(|d| d.as_array())
                        {
                            for msg in messages {
                                let msg_id = msg.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                                if msg_id <= last_known_id {
                                    continue;
                                }
                                last_known_id = msg_id;

                                let content =
                                    msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                let actor =
                                    msg.get("actorId").and_then(|v| v.as_str()).unwrap_or("");

                                // Skip bot's own messages
                                if actor == nc_config.username {
                                    continue;
                                }

                                if !nc_config
                                    .allowlist
                                    .is_allowed(Some(actor), Some(room_token))
                                {
                                    continue;
                                }

                                if content.is_empty() {
                                    continue;
                                }

                                let session_key = room_token.clone();
                                let session = agent_session.clone();
                                let client = client.clone();
                                let reply_url = format!(
                                    "{}/ocs/v2.php/apps/spreed/api/v1/chat/{}",
                                    base_url, room_token
                                );
                                let username = nc_config.username.clone();
                                let pw = password.clone();
                                let msg_text = content.to_string();
                                let sender_id = actor.to_string();

                                tokio::spawn(async move {
                                    let channel_info = ChannelInfo {
                                        platform: "nextcloud".into(),
                                        native_channel_id: Some(session_key.clone()),
                                        ..Default::default()
                                    };
                                    let sender_info = SenderInfo {
                                        id: Some(sender_id),
                                        ..Default::default()
                                    };
                                    let chat_info = ChatInfo {
                                        chat_type: "group".into(),
                                        ..Default::default()
                                    };
                                    let mut msg = InboundMessage::channel(
                                        session_key.clone(),
                                        msg_text.clone(),
                                        channel_info,
                                        sender_info,
                                        chat_info,
                                    );
                                    msg.finalize();
                                    match session.handle_message(msg, RunContext::default()).await {
                                        Ok(reply) => {
                                            let _ = client
                                                .post(&reply_url)
                                                .basic_auth(&username, Some(&pw))
                                                .header("OCS-APIRequest", "true")
                                                .json(
                                                    &serde_json::json!({"message": reply.content}),
                                                )
                                                .send()
                                                .await;
                                        }
                                        Err(e) => {
                                            tracing::error!(channel = "nextcloud", error = %e, "agent error");
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                Ok(r) => {
                    tracing::warn!(channel = "nextcloud", status = %r.status(), room = %room_token, "unexpected status");
                }
                Err(e) => {
                    tracing::warn!(channel = "nextcloud", error = %e, "request error");
                }
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`NextcloudAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Nextcloud Talk bot.
#[allow(dead_code)]
pub struct NextcloudAdapter {
    /// HTTP client for making API calls.
    client: Client,
    /// Base URL of the Nextcloud instance, e.g. `https://cloud.example.com`.
    base_url: String,
    /// Nextcloud username used for Basic Auth.
    username: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl NextcloudAdapter {
    pub fn new(base_url: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            username: username.into(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for NextcloudAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "nextcloud".to_string(),
            name: "Nextcloud Talk".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(32000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "nextcloud", "NextcloudAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "nextcloud", "NextcloudAdapter stopped");
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
impl Outbound for NextcloudAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Placeholder: a full implementation would POST to
        // `{base_url}/ocs/v2.php/apps/spreed/api/v1/chat/{room_token}`
        // using Basic Auth with `self.username`.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for NextcloudAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!(
            "{}/ocs/v2.php/apps/spreed/api/v1/room",
            self.base_url.trim_end_matches('/')
        );
        match self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(""))
            .header("OCS-APIRequest", "true")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => {
                // 401 means the server is reachable; credentials may just be wrong.
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("health probe returned HTTP {}", resp.status());
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
