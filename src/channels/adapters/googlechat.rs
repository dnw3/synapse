use std::sync::Arc;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::{BotAllowlist, SynapseConfig};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Json;
use axum::Router;

/// Shared state for the axum webhook server.
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
}

/// Incoming Google Chat event payload (subset of fields we care about).
///
/// Reference: https://developers.google.com/chat/api/reference/rest/v1/Event
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GoogleChatEvent {
    /// Event type: "MESSAGE", "ADDED_TO_SPACE", "REMOVED_FROM_SPACE", "CARD_CLICKED", etc.
    #[serde(rename = "type")]
    event_type: Option<String>,
    /// The message that triggered the event (present for MESSAGE events).
    message: Option<ChatMessage>,
    /// The space in which the event occurred.
    space: Option<Space>,
}

/// A Google Chat message object (subset).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ChatMessage {
    /// The resource name of the message (e.g. "spaces/xxx/messages/yyy").
    name: Option<String>,
    /// Plain-text body of the message.
    text: Option<String>,
    /// The user who sent the message.
    sender: Option<User>,
}

/// A Google Chat user object.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct User {
    /// Resource name of the user (e.g. "users/12345").
    name: Option<String>,
    /// Display name of the user.
    display_name: Option<String>,
}

/// A Google Chat space object.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Space {
    /// Resource name of the space (e.g. "spaces/AAAA1234").
    name: Option<String>,
    /// Display name of the space.
    display_name: Option<String>,
}

/// Synchronous reply body returned to Google Chat.
///
/// Google Chat reads the HTTP response body as the bot reply when
/// the bot handles events synchronously (within 30 seconds).
#[derive(Debug, serde::Serialize)]
struct TextReply {
    text: String,
}

/// Handle incoming Google Chat webhook POST at `/`.
///
/// Google Chat sends all event types (MESSAGE, ADDED_TO_SPACE, etc.) to the
/// same endpoint. We only act on MESSAGE events; everything else returns 200 OK
/// with an empty body so Google Chat does not report an error.
async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    Json(event): Json<GoogleChatEvent>,
) -> Result<Json<TextReply>, StatusCode> {
    // Only process MESSAGE events.
    let event_type = event.event_type.as_deref().unwrap_or("");
    if event_type != "MESSAGE" {
        return Err(StatusCode::OK);
    }

    let message = match event.message.as_ref() {
        Some(m) => m,
        None => return Err(StatusCode::OK),
    };

    let text = match message.text.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return Err(StatusCode::OK),
    };

    let sender_name = message
        .sender
        .as_ref()
        .and_then(|u| u.name.as_deref())
        .map(|s| s.to_string());

    let space_name = event
        .space
        .as_ref()
        .and_then(|s| s.name.as_deref())
        .map(|s| s.to_string());

    // Allowlist check: filter by sender resource name or space resource name.
    if !state
        .allowlist
        .is_allowed(sender_name.as_deref(), space_name.as_deref())
    {
        // Silently ignore — return 200 with no text so Google Chat doesn't retry.
        return Err(StatusCode::OK);
    }

    // Session key scoped to the space (group chat) or sender (DM).
    let session_key = if let Some(ref space) = space_name {
        format!("googlechat:{}", space)
    } else if let Some(ref sender) = sender_name {
        format!("googlechat:{}", sender)
    } else {
        "googlechat:unknown".to_string()
    };

    // Process the message through the agent session.
    let reply_text = match state
        .agent_session
        .handle_message(&session_key, &text)
        .await
    {
        Ok(reply) => {
            let chunks = formatter::chunk_googlechat(&reply);
            // Google Chat synchronous replies support only a single text body.
            // If the response is chunked, join all chunks separated by a blank line.
            if chunks.is_empty() {
                reply
            } else {
                chunks.join("\n\n")
            }
        }
        Err(e) => {
            tracing::error!(channel = "googlechat", error = %e, "handler error");
            format!("Error: {}", e)
        }
    };

    Ok(Json(TextReply { text: reply_text }))
}

/// Run the Google Chat bot adapter (webhook mode).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let gchat_config = config
        .googlechat
        .as_ref()
        .ok_or("missing [googlechat] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = gchat_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = gchat_config.port.unwrap_or(8077);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "googlechat",
            senders = allowlist.allowed_users.len(),
            spaces = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    if let Some(ref project_id) = gchat_config.project_id {
        tracing::info!(channel = "googlechat", project_id = %project_id, "project configured");
    }

    tracing::info!(
        channel = "googlechat",
        port = port,
        "adapter started (webhook mode)"
    );

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
    });

    let app = Router::new()
        .route("/", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!(channel = "googlechat", addr = %addr, "listening");

    axum::serve(listener, app).await?;

    Ok(())
}
