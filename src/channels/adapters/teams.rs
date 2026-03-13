use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

/// Shared state for the axum webhook server.
#[allow(dead_code)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
    app_id: String,
    app_password: String,
}

/// Bot Framework Activity payload (subset of fields we care about).
///
/// Reference: https://learn.microsoft.com/en-us/azure/bot-service/rest-api/bot-framework-rest-connector-api-reference
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Activity {
    /// Activity type: "message", "conversationUpdate", "event", etc.
    #[serde(rename = "type")]
    activity_type: Option<String>,
    /// Unique activity ID.
    id: Option<String>,
    /// Service URL to reply to.
    service_url: Option<String>,
    /// Channel ID (e.g. "msteams", "emulator").
    channel_id: Option<String>,
    /// Conversation reference.
    conversation: Option<ConversationRef>,
    /// Sender account.
    from: Option<ChannelAccount>,
    /// Recipient (the bot's account).
    recipient: Option<ChannelAccount>,
    /// Text content of the message.
    text: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConversationRef {
    id: Option<String>,
    #[serde(default)]
    is_group: bool,
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct ChannelAccount {
    id: Option<String>,
    name: Option<String>,
}

/// Reply payload sent back to the Bot Framework REST API.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplyActivity {
    #[serde(rename = "type")]
    activity_type: &'static str,
    text: String,
    conversation: Option<ConversationRef>,
    from: Option<ChannelAccount>,
    recipient: Option<ChannelAccount>,
    reply_to_id: Option<String>,
}

/// Build the Bot Framework REST API URL for sending a reply.
///
/// Pattern: `{serviceUrl}/v3/conversations/{conversationId}/activities`
fn reply_url(service_url: &str, conversation_id: &str) -> String {
    let base = service_url.trim_end_matches('/');
    format!("{}/v3/conversations/{}/activities", base, conversation_id)
}

/// Obtain a Bearer token from Azure AD using the client-credentials flow.
///
/// Bot Framework uses: POST https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token
async fn fetch_bot_token(
    client: &reqwest::Client,
    app_id: &str,
    app_password: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let params = [
        ("grant_type", "client_credentials"),
        ("client_id", app_id),
        ("client_secret", app_password),
        ("scope", "https://api.botframework.com/.default"),
    ];

    let resp = client
        .post("https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    let token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("no access_token in Azure AD response")?
        .to_string();

    Ok(token)
}

/// Send a reply message via the Bot Framework REST API.
async fn send_reply(
    client: &reqwest::Client,
    app_id: &str,
    app_password: &str,
    service_url: &str,
    activity: &Activity,
    text: &str,
) {
    // Obtain OAuth token for this call.
    let token = match fetch_bot_token(client, app_id, app_password).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(channel = "teams", error = %e, "failed to obtain bot token");
            return;
        }
    };

    let conversation_id = activity
        .conversation
        .as_ref()
        .and_then(|c| c.id.as_deref())
        .unwrap_or_default();

    if conversation_id.is_empty() {
        tracing::warn!(
            channel = "teams",
            "no conversation ID in activity, cannot reply"
        );
        return;
    }

    let url = reply_url(service_url, conversation_id);

    let reply = ReplyActivity {
        activity_type: "message",
        text: text.to_string(),
        conversation: activity.conversation.clone(),
        // Swap from/recipient for the reply direction
        from: activity.recipient.clone(),
        recipient: activity.from.clone(),
        reply_to_id: activity.id.clone(),
    };

    if let Err(e) = client
        .post(&url)
        .bearer_auth(&token)
        .json(&reply)
        .send()
        .await
    {
        tracing::error!(channel = "teams", error = %e, "failed to send reply");
    }
}

/// Handle incoming Bot Framework activity POST at `/api/messages`.
async fn handle_message(
    State(state): State<Arc<AppState>>,
    axum::Json(activity): axum::Json<Activity>,
) -> StatusCode {
    // Only handle "message" activity types.
    let activity_type = activity.activity_type.as_deref().unwrap_or("");

    if activity_type != "message" {
        // Accept but do nothing (conversationUpdate, typing, etc.)
        return StatusCode::OK;
    }

    let text = match activity.text.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return StatusCode::OK,
    };

    let user_id = activity
        .from
        .as_ref()
        .and_then(|f| f.id.as_deref())
        .map(|s| s.to_string());

    let conversation_id = activity
        .conversation
        .as_ref()
        .and_then(|c| c.id.as_deref())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let service_url = match activity.service_url.as_deref() {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            tracing::warn!(channel = "teams", "no serviceUrl in activity, cannot reply");
            return StatusCode::OK;
        }
    };

    // Allowlist check.
    if !state.allowlist.is_allowed(
        user_id.as_deref(),
        if conversation_id.is_empty() {
            None
        } else {
            Some(&conversation_id)
        },
    ) {
        return StatusCode::OK; // Silently ignore unauthorized
    }

    // Session key scoped to the Teams conversation.
    let session_key = if conversation_id.is_empty() {
        format!("teams:{}", user_id.as_deref().unwrap_or("unknown"))
    } else {
        format!("teams:{}", conversation_id)
    };

    // Process in the background so we respond to Bot Framework promptly (< 5 s).
    let session = state.agent_session.clone();
    let app_id = state.app_id.clone();
    let app_password = state.app_password.clone();
    // We need to clone the activity fields we'll need inside the spawn.
    let activity_id = activity.id.clone();
    let conversation = activity.conversation.clone();
    let from = activity.from.clone();
    let recipient = activity.recipient.clone();

    // Reconstruct a minimal activity for send_reply.
    let activity_ref = Activity {
        activity_type: activity.activity_type.clone(),
        id: activity_id,
        service_url: Some(service_url.clone()),
        channel_id: activity.channel_id.clone(),
        conversation,
        from,
        recipient,
        text: Some(text.clone()),
    };

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        let envelope = MessageEnvelope::channel(
            session_key.clone(),
            text.clone(),
            DeliveryContext {
                channel: "teams".into(),
                to: Some(format!("conversation:{}", session_key)),
                account_id: None,
                thread_id: None,
                meta: None,
            },
        );
        match session.handle_message(envelope).await {
            Ok(reply) => {
                let chunks = formatter::chunk_teams(&reply.content);
                for chunk in &chunks {
                    send_reply(
                        &client,
                        &app_id,
                        &app_password,
                        &service_url,
                        &activity_ref,
                        chunk,
                    )
                    .await;
                }
            }
            Err(e) => {
                tracing::error!(channel = "teams", error = %e, "handler error");
                // Attempt to relay the error back to the user.
                send_reply(
                    &client,
                    &app_id,
                    &app_password,
                    &service_url,
                    &activity_ref,
                    &format!("Error: {}", e),
                )
                .await;
            }
        }
    });

    StatusCode::OK
}

/// Run the Microsoft Teams bot adapter (Bot Framework webhook mode).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let teams_config = config
        .teams
        .as_ref()
        .ok_or("missing [teams] section in config")?;

    let app_password = resolve_secret(
        teams_config.app_password.as_deref(),
        teams_config.app_password_env.as_deref(),
        "Teams app password",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = teams_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true).with_channel("teams"));

    let port = teams_config.port.unwrap_or(3978);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "teams",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(
        channel = "teams",
        app_id = %teams_config.app_id,
        port = port,
        "adapter started"
    );

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
        app_id: teams_config.app_id.clone(),
        app_password,
    });

    let app = Router::new()
        .route("/api/messages", post(handle_message))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!(channel = "teams", addr = %addr, "listening");

    axum::serve(listener, app).await?;

    Ok(())
}
