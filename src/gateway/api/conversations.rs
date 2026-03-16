use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use serde::Serialize;
use synaptic::core::MemoryStore;
use synaptic::events::{Event, EventKind};
use tracing;

use crate::gateway::state::AppState;

fn parse_system_time_string(s: &str) -> String {
    if let Some(sec_start) = s.find("tv_sec: ") {
        let rest = &s[sec_start + 8..];
        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(secs) = rest[..end].parse::<u64>() {
                return (secs * 1000).to_string();
            }
        }
    }
    s.to_string()
}

/// Parse a session key to extract channel, kind, and display_name.
///
/// Key formats (from session_key.rs):
///   `agent:{agent_id}:main`                                    → web/main
///   `agent:{agent_id}:{channel}:dm:{peer_id}`                  → channel/dm
///   `agent:{agent_id}:{channel}:{account}:dm:{peer_id}`        → channel/dm (multi-account)
///   `agent:{agent_id}:{channel}:grp:{peer_id}[:{extras}]`      → channel/group
///   UUID (web sessions)                                         → web
pub fn parse_session_channel(id: &str) -> (String, String, String) {
    if !id.starts_with("agent:") {
        return ("web".to_string(), "web".to_string(), String::new());
    }
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() == 3 && parts[2] == "main" {
        return ("web".to_string(), "main".to_string(), "main".to_string());
    }
    for (i, part) in parts.iter().enumerate() {
        if *part == "dm" && i >= 2 {
            let channel = parts[2].to_string();
            let peer = if i + 1 < parts.len() {
                parts[i + 1]
            } else {
                ""
            };
            return (channel, "dm".to_string(), peer.to_string());
        }
        if *part == "grp" && i >= 2 {
            let channel = parts[2].to_string();
            let peer = if i + 1 < parts.len() {
                parts[i + 1..].join(":")
            } else {
                String::new()
            };
            return (channel, "group".to_string(), peer);
        }
    }
    ("web".to_string(), "web".to_string(), String::new())
}

#[derive(Serialize)]
struct ConversationResponse {
    id: String,
    created_at: String,
    message_count: usize,
    channel: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_count: Option<u64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/conversations", post(create_conversation))
        .route("/conversations", get(list_conversations))
        .route("/conversations/{id}", get(get_conversation))
        .route("/conversations/{id}", delete(delete_conversation))
}

async fn create_conversation(
    State(state): State<AppState>,
) -> Result<Json<ConversationResponse>, (StatusCode, String)> {
    const MAIN_SESSION_KEY: &str = "agent:default:main";

    // Check if the main web session already exists — reuse it if so.
    let existing = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .find(|s| s.session_key.as_deref() == Some(MAIN_SESSION_KEY));

    let info = if let Some(existing_info) = existing {
        existing_info
    } else {
        // Create a new session and tag it with the main session key.
        let session_id = state
            .sessions
            .create_session()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut info = state
            .sessions
            .get_session(&session_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .unwrap_or_else(|| synaptic::session::SessionInfo {
                session_id: session_id.clone(),
                created_at: String::new(),
                ..Default::default()
            });

        info.session_key = Some(MAIN_SESSION_KEY.to_string());
        info.channel = Some("web".to_string());
        info.chat_type = Some("direct".to_string());
        info.display_name = Some("main".to_string());

        state
            .sessions
            .update_session(&info)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        info
    };

    tracing::info!(session_key = ?info.session_key, "conversation created");

    let channel = info.channel.clone().unwrap_or_else(|| "web".to_string());
    let kind = info
        .chat_type
        .clone()
        .unwrap_or_else(|| "direct".to_string());
    let display_name = info.display_name.clone();
    Ok(Json(ConversationResponse {
        id: info.session_id,
        created_at: parse_system_time_string(&info.created_at),
        message_count: 0,
        channel,
        kind,
        display_name,
        title: None,
        token_count: Some(info.total_tokens),
    }))
}

async fn list_conversations(
    State(state): State<AppState>,
) -> Result<Json<Vec<ConversationResponse>>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory = state.sessions.memory();
    let mut conversations = Vec::with_capacity(sessions.len());
    for s in sessions {
        let messages = memory.load(&s.session_id).await.unwrap_or_default();
        let count = messages.len();
        // Use SessionInfo fields directly; fall back to parse_session_channel for legacy sessions.
        let (channel, kind, display_name) = if s.channel.is_some() || s.chat_type.is_some() {
            (
                s.channel.clone().unwrap_or_else(|| "web".to_string()),
                s.chat_type.clone().unwrap_or_else(|| "web".to_string()),
                s.display_name.clone(),
            )
        } else {
            let (ch, k, dn) = parse_session_channel(&s.session_id);
            (ch, k, if dn.is_empty() { None } else { Some(dn) })
        };
        conversations.push(ConversationResponse {
            id: s.session_id,
            created_at: parse_system_time_string(&s.created_at),
            message_count: count,
            channel,
            kind,
            display_name,
            title: None,
            token_count: Some(s.total_tokens),
        });
    }

    let count = conversations.len();
    tracing::debug!(count, "conversations listed");

    Ok(Json(conversations))
}

async fn get_conversation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ConversationResponse>, (StatusCode, String)> {
    let info = state
        .sessions
        .get_session(&id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session '{}' not found", id)))?;

    let memory = state.sessions.memory();
    let messages = memory.load(&id).await.unwrap_or_default();
    let count = messages.len();
    // Use SessionInfo fields directly; fall back to parse_session_channel for legacy sessions.
    let (channel, kind, display_name) = if info.channel.is_some() || info.chat_type.is_some() {
        (
            info.channel.clone().unwrap_or_else(|| "web".to_string()),
            info.chat_type.clone().unwrap_or_else(|| "web".to_string()),
            info.display_name.clone(),
        )
    } else {
        let (ch, k, dn) = parse_session_channel(&info.session_id);
        (ch, k, if dn.is_empty() { None } else { Some(dn) })
    };
    Ok(Json(ConversationResponse {
        id: info.session_id,
        created_at: parse_system_time_string(&info.created_at),
        message_count: count,
        channel,
        kind,
        display_name,
        title: None,
        token_count: Some(info.total_tokens),
    }))
}

async fn delete_conversation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .sessions
        .delete_session(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(conversation_id = %id, "conversation deleted");

    // Emit SessionEnd (fire-and-forget)
    {
        let event_bus = state.event_bus.clone();
        let session_id = id.clone();
        tokio::spawn(async move {
            let mut event = Event::new(
                EventKind::SessionEnd,
                serde_json::json!({ "session_id": session_id }),
            )
            .with_source("gateway/api");
            let _ = event_bus.emit(&mut event).await;
        });
    }

    Ok(StatusCode::NO_CONTENT)
}
