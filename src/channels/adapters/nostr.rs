use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::time::Duration;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{NostrBotConfig, SynapseConfig};

/// Run the Nostr bot adapter.
///
/// Connects to Nostr relays via WebSocket (NIP-01 protocol) and responds to
/// direct messages (NIP-04 kind 4) and mentions.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let nostr_config = config
        .nostr
        .first()
        .ok_or("missing [[nostr]] section in config")?;

    let private_key = resolve_secret(
        nostr_config.private_key.as_deref(),
        nostr_config.private_key_env.as_deref(),
        "Nostr private key",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    tracing::info!(
        channel = "nostr",
        relays = nostr_config.relays.len(),
        "adapter started"
    );

    // Simple polling loop using NIP-50 search or NIP-01 REQ subscriptions
    // via relay HTTP/WebSocket. For production, use nostr-sdk crate.
    loop {
        for relay in &nostr_config.relays {
            if let Err(e) =
                poll_relay(relay, &private_key, &agent_session, &nostr_config.allowlist).await
            {
                tracing::warn!(channel = "nostr", relay = %relay, error = %e, "relay error");
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn poll_relay(
    _relay_url: &str,
    _private_key: &str,
    _agent_session: &Arc<AgentSession>,
    _allowlist: &crate::config::BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder: In a full implementation, this would:
    // 1. Connect to relay via WebSocket
    // 2. Subscribe to kind 4 (DM) and kind 1 (text note) events mentioning the bot
    // 3. Decrypt DMs using NIP-04
    // 4. Process messages through AgentSession
    // 5. Publish response events

    // For now, this is a skeleton that compiles and can be filled in
    // when nostr-sdk or a similar crate is added as a dependency.
    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`NostrAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Nostr relay bot.
#[allow(dead_code)]
pub struct NostrAdapter {
    /// List of relay WebSocket URLs.
    relays: Vec<String>,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl NostrAdapter {
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            relays,
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for NostrAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "nostr".to_string(),
            name: "Nostr".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
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
        tracing::info!(channel = "nostr", "NostrAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "nostr", "NostrAdapter stopped");
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
impl Outbound for NostrAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Placeholder: a full implementation would publish a NIP-01 event
        // (kind 1 note or kind 4 DM) to each configured relay via WebSocket.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for NostrAdapter {
    async fn health_check(&self) -> HealthStatus {
        if self.relays.is_empty() {
            self.status.store(STATUS_ERROR, Ordering::SeqCst);
            return HealthStatus::Unhealthy("no relays configured".to_string());
        }
        // Report healthy if at least one relay URL is present; a real
        // implementation would probe the WebSocket connection.
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        HealthStatus::Healthy
    }
}
