use std::sync::Arc;

use reqwest::Client;
use tokio::time::Duration;

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
        .as_ref()
        .ok_or("missing [tlon] section in config")?;

    let api_key = resolve_secret(tlon_config.api_key.as_deref(), tlon_config.api_key_env.as_deref(), "Tlon API key")
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
