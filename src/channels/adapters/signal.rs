use std::sync::Arc;
use std::time::Duration;

use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::SynapseConfig;
use crate::gateway::messages::MessageEnvelope;

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
        .as_ref()
        .ok_or("missing [signal] section in config")?;

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
                let envelope = MessageEnvelope::channel(
                    recipient.clone(),
                    text.clone(),
                    DeliveryContext {
                        channel: "signal".into(),
                        to: Some(format!("user:{}", recipient)),
                        account_id: None,
                        thread_id: None,
                        meta: None,
                    },
                );
                match session.handle_message(envelope).await {
                    Ok(reply) => {
                        let chunks = formatter::chunk_signal(&reply.content);
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
