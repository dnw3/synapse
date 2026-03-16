use std::sync::atomic::{AtomicU8, Ordering};

use async_trait::async_trait;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

/// Status constants used by [`BlueBubblesAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// BlueBubbles-based iMessage adapter.
///
/// Works on any platform via a BlueBubbles server (runs on macOS with iMessage).
/// Sends outbound messages via `POST /api/v1/message/text` and checks server
/// health via `GET /api/v1/server/info`.
#[allow(dead_code)]
pub struct BlueBubblesAdapter {
    client: reqwest::Client,
    base_url: String,
    password: String,
    status: AtomicU8,
}

#[allow(dead_code)]
impl BlueBubblesAdapter {
    pub fn new(base_url: String, password: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            password,
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for BlueBubblesAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "bluebubbles".into(),
            name: "iMessage (BlueBubbles)".into(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(20000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: true,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(
            channel = "bluebubbles",
            "BlueBubbles iMessage adapter started"
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(
            channel = "bluebubbles",
            "BlueBubbles iMessage adapter stopped"
        );
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
impl Outbound for BlueBubblesAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        let url = format!("{}/api/v1/message/text", self.base_url);
        let _resp = self
            .client
            .post(&url)
            .query(&[("password", &self.password)])
            .json(&serde_json::json!({
                "chatGuid": envelope.channel_id,
                "message": envelope.content,
            }))
            .send()
            .await
            .map_err(|e| {
                synaptic::core::SynapticError::Tool(format!("BlueBubbles send failed: {}", e))
            })?;
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for BlueBubblesAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/api/v1/server/info", self.base_url);
        match self
            .client
            .get(&url)
            .query(&[("password", &self.password)])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("BlueBubbles health returned HTTP {}", resp.status());
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
