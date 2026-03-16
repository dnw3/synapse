use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use tokio::time::Duration;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{SynapseConfig, TlonBotConfig};

/// Run the Tlon (Urbit) bot adapter using HTTP SSE polling.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tlon_config = config
        .tlon
        .first()
        .ok_or("missing [[tlon]] section in config")?;

    let api_key = resolve_secret(
        tlon_config.api_key.as_deref(),
        tlon_config.api_key_env.as_deref(),
        "Tlon API key",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let client = Client::new();
    let base_url = tlon_config.url.trim_end_matches('/');

    tracing::info!(channel = "tlon", url = %base_url, "adapter started (polling mode)");

    let poll_interval = Duration::from_secs(5);

    loop {
        // Poll for new messages via Tlon HTTP API
        // The Tlon API uses SSE for real-time updates; this is a simplified polling fallback.
        if let Err(e) = poll_messages(
            &client,
            base_url,
            &api_key,
            &agent_session,
            &tlon_config.allowlist,
        )
        .await
        {
            tracing::warn!(channel = "tlon", error = %e, "poll error");
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn poll_messages(
    _client: &Client,
    _base_url: &str,
    _api_key: &str,
    _agent_session: &Arc<AgentSession>,
    _allowlist: &crate::config::BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder: In a full implementation, this would:
    // 1. Subscribe to Tlon channel SSE events
    // 2. Parse incoming chat messages
    // 3. Process through AgentSession
    // 4. POST reply back via Tlon API
    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`TlonAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Tlon (Urbit) bot.
#[allow(dead_code)]
pub struct TlonAdapter {
    /// HTTP client for making API calls.
    client: Client,
    /// Base URL of the Tlon server, e.g. `https://tlon.network`.
    base_url: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl TlonAdapter {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for TlonAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "tlon".to_string(),
            name: "Tlon".to_string(),
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
        tracing::info!(channel = "tlon", "TlonAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "tlon", "TlonAdapter stopped");
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
impl Outbound for TlonAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Placeholder: a full implementation would POST a message to the
        // Tlon channel API at `self.base_url`.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for TlonAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() < 500 => {
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
