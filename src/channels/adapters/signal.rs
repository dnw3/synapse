use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::SynapseConfig;
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

/// Run the Signal bot adapter using the signal-cli REST API bridge.
///
/// Polls `GET /v1/receive/{number}` for incoming messages and replies via
/// `POST /v2/send`.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let signal_config = config
        .signal
        .first()
        .ok_or("missing [[signal]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = signal_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let api_url = signal_config.api_url.trim_end_matches('/').to_string();
    let phone_number = signal_config.phone_number.clone();

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "signal",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "signal", "adapter started");
    tracing::info!(
        channel = "signal",
        api_url = %api_url,
        phone_number = %phone_number,
        "polling started"
    );

    let client = reqwest::Client::new();
    let receive_url = format!(
        "{}/v1/receive/{}",
        api_url,
        urlencoding::encode(&phone_number)
    );

    loop {
        let resp = match client.get(&receive_url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(channel = "signal", error = %e, "polling error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let envelopes: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(channel = "signal", error = %e, "response parse error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let messages = match envelopes.as_array() {
            Some(arr) => arr.clone(),
            None => {
                // Empty or unexpected response — wait before next poll
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        if messages.is_empty() {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        for envelope in messages {
            // The envelope has a nested dataMessage for regular messages
            let data_message = match envelope.get("envelope").and_then(|e| e.get("dataMessage")) {
                Some(dm) => dm,
                None => continue,
            };

            let text = match data_message.get("message").and_then(|v| v.as_str()) {
                Some(t) if !t.is_empty() => t.to_string(),
                _ => continue,
            };

            // The sender phone number — used as both user_id and session key
            let sender = envelope
                .get("envelope")
                .and_then(|e| e.get("source"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let sender = match sender {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };

            // Allowlist check — Signal has no channels; check by sender number only
            if !allowlist.is_allowed(Some(&sender), None) {
                continue;
            }

            // Process message in background
            let session = agent_session.clone();
            let http = client.clone();
            let send_url = format!("{}/v2/send", api_url);
            let our_number = phone_number.clone();
            let recipient = sender.clone();

            tokio::spawn(async move {
                let channel_info = ChannelInfo {
                    platform: "signal".into(),
                    ..Default::default()
                };
                let sender_info = SenderInfo {
                    id: Some(recipient.clone()),
                    e164: Some(recipient.clone()),
                    ..Default::default()
                };
                let chat_info = ChatInfo {
                    chat_type: "direct".into(),
                    ..Default::default()
                };
                let mut msg = InboundMessage::channel(
                    recipient.clone(),
                    text.clone(),
                    channel_info,
                    sender_info,
                    chat_info,
                );
                msg.finalize();
                match session.handle_message(msg).await {
                    Ok(reply) => {
                        let chunks = formatter::format_for_channel(&reply.content, "signal", 4096);
                        for chunk in chunks {
                            let body = serde_json::json!({
                                "message": chunk,
                                "number": our_number,
                                "recipients": [recipient],
                            });
                            if let Err(e) = http.post(&send_url).json(&body).send().await {
                                tracing::error!(channel = "signal", error = %e, "send error");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(channel = "signal", error = %e, "handler error");
                    }
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`SignalAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Signal CLI REST bridge.
#[allow(dead_code)]
pub struct SignalAdapter {
    api_url: String,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl SignalAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for SignalAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "signal".to_string(),
            name: "Signal".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(40000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "signal", "SignalAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "signal", "SignalAdapter stopped");
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
impl Outbound for SignalAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "signal",
            to = %envelope.channel_id,
            "SignalAdapter::send (placeholder)",
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for SignalAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/v1/health", self.api_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("signal-cli health returned HTTP {}", resp.status());
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
