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
fn parse_session_channel(id: &str) -> (String, String, String) {
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
    let session_id = state
        .sessions
        .create_session()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch the newly created session info for created_at
    let info = state
        .sessions
        .get_session(&session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or_else(|| synaptic::session::SessionInfo {
            id: session_id.clone(),
            created_at: String::new(),
            compaction_count: 0,
            token_count: 0,
        });

    tracing::info!("conversation created");

    let (channel, kind, display_name) = parse_session_channel(&info.id);
    Ok(Json(ConversationResponse {
        id: info.id,
        created_at: parse_system_time_string(&info.created_at),
        message_count: 0,
        channel,
        kind,
        display_name: if display_name.is_empty() {
            None
        } else {
            Some(display_name)
        },
        title: None,
        token_count: Some(info.token_count),
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
        let messages = memory.load(&s.id).await.unwrap_or_default();
        let count = messages.len();
        let title = messages.iter().find(|m| m.is_human()).map(|m| {
            let content = m.content();
            if content.chars().count() > 60 {
                format!("{}...", content.chars().take(60).collect::<String>())
            } else {
                content.to_string()
            }
        });
        let (channel, kind, display_name) = parse_session_channel(&s.id);
        conversations.push(ConversationResponse {
            id: s.id,
            created_at: parse_system_time_string(&s.created_at),
            message_count: count,
            channel,
            kind,
            display_name: if display_name.is_empty() {
                None
            } else {
                Some(display_name)
            },
            title,
            token_count: Some(s.token_count),
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
    let title = messages.iter().find(|m| m.is_human()).map(|m| {
        let content = m.content();
        if content.chars().count() > 60 {
            format!("{}...", content.chars().take(60).collect::<String>())
        } else {
            content.to_string()
        }
    });
    let (channel, kind, display_name) = parse_session_channel(&info.id);
    Ok(Json(ConversationResponse {
        id: info.id,
        created_at: parse_system_time_string(&info.created_at),
        message_count: count,
        channel,
        kind,
        display_name: if display_name.is_empty() {
            None
        } else {
            Some(display_name)
        },
        title,
        token_count: Some(info.token_count),
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
