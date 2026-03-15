use std::sync::Arc;

use reqwest::Client;
use tokio::time::Duration;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{NextcloudBotConfig, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

/// Run the Nextcloud Talk bot adapter using REST long-polling.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let nc_config = config
        .nextcloud
        .first()
        .ok_or("missing [[nextcloud]] section in config")?;

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
                                    let mut envelope = MessageEnvelope::channel(
                                        session_key.clone(),
                                        msg_text.clone(),
                                        DeliveryContext {
                                            channel: "nextcloud".into(),
                                            to: Some(format!("room:{}", session_key)),
                                            account_id: None,
                                            thread_id: None,
                                            meta: None,
                                        },
                                    );
                                    envelope.sender_id = Some(sender_id);
                                    envelope.routing.peer_kind =
                                        Some(crate::config::PeerKind::Group);
                                    envelope.routing.peer_id = Some(session_key.clone());
                                    match session.handle_message(envelope).await {
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
