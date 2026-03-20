use axum::extract::ws::Message as WsMessage;
use synaptic::core::{MemoryStore, Message};

use super::types::WsEvent;
use crate::agent::SessionOverrides;
use crate::gateway::state::AppState;

#[allow(dead_code)]
pub(crate) fn ws_json(event: &WsEvent) -> WsMessage {
    WsMessage::Text(serde_json::to_string(event).unwrap().into())
}

#[allow(dead_code)]
pub(crate) fn find_tool_name(messages: &[Message], displayed: usize, tool_msg: &Message) -> String {
    let tool_call_id = tool_msg.tool_call_id().unwrap_or_default();
    if tool_call_id.is_empty() {
        return "tool".to_string();
    }
    for msg in messages[..displayed].iter().rev() {
        if msg.is_ai() {
            for tc in msg.tool_calls() {
                if tc.id == tool_call_id {
                    return tc.name.clone();
                }
            }
        }
    }
    "tool".to_string()
}

#[allow(dead_code)]
pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

/// Handle an RPC request from the client (legacy protocol).
#[allow(dead_code)]
pub(crate) async fn handle_rpc(
    state: &AppState,
    conversation_id: &str,
    method: &str,
    _params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method {
        "get_status" => {
            let uptime = state.started_at.elapsed().as_secs();
            let auth_enabled = state
                .auth
                .as_ref()
                .map(|a| a.config.enabled)
                .unwrap_or(false);
            Ok(serde_json::json!({
                "status": "ok",
                "uptime_secs": uptime,
                "auth_enabled": auth_enabled,
                "conversation_id": conversation_id,
            }))
        }
        "get_messages" => {
            let memory = state.sessions.memory();
            let messages = memory.load(conversation_id).await.unwrap_or_default();
            let msg_list: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": if m.is_human() { "human" } else if m.is_ai() { "assistant" } else if m.is_system() { "system" } else { "tool" },
                        "content": m.content(),
                    })
                })
                .collect();
            Ok(serde_json::json!({ "messages": msg_list }))
        }
        "get_session_info" => {
            let memory = state.sessions.memory();
            let messages = memory.load(conversation_id).await.unwrap_or_default();
            let overrides = load_session_overrides(conversation_id);
            Ok(serde_json::json!({
                "conversation_id": conversation_id,
                "message_count": messages.len(),
                "thinking": overrides.as_ref().and_then(|o| o.thinking.as_deref()),
            }))
        }
        "check_execution" => {
            let is_executing = state.write_lock.is_locked(conversation_id).await;
            Ok(serde_json::json!({ "executing": is_executing }))
        }
        _ => Err(format!("unknown method: {}", method)),
    }
}

/// Load session overrides (thinking/verbose) from the dashboard overrides file.
pub(crate) fn load_session_overrides(conversation_id: &str) -> Option<SessionOverrides> {
    let path = std::path::PathBuf::from("data/session_overrides.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&content).ok()?;
    let entry = map.get(conversation_id)?;
    let thinking = entry
        .get("thinking")
        .and_then(|v| v.as_str())
        .map(String::from);
    let verbose = entry
        .get("verbose")
        .and_then(|v| v.as_str())
        .map(String::from);
    if thinking.is_none() && verbose.is_none() {
        return None;
    }
    Some(SessionOverrides { thinking, verbose })
}
