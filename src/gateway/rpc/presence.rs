//! RPC handlers for presence and system events.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_system_presence(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let text = params
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let entry = crate::gateway::presence::PresenceEntry {
        key: String::new(),
        host: params
            .get("host")
            .and_then(|v| v.as_str())
            .map(String::from),
        ip: params.get("ip").and_then(|v| v.as_str()).map(String::from),
        version: params
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from),
        platform: params
            .get("platform")
            .and_then(|v| v.as_str())
            .map(String::from),
        device_family: params
            .get("device_family")
            .and_then(|v| v.as_str())
            .map(String::from),
        model_identifier: params
            .get("model_identifier")
            .and_then(|v| v.as_str())
            .map(String::from),
        mode: params
            .get("mode")
            .and_then(|v| v.as_str())
            .map(String::from),
        reason: params
            .get("reason")
            .and_then(|v| v.as_str())
            .map(String::from),
        device_id: params
            .get("device_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        instance_id: params
            .get("instance_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        roles: params
            .get("roles")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        scopes: params
            .get("scopes")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        text,
        ts: crate::gateway::presence::now_ms(),
    };

    let changed = ctx.state.network.presence.write().await.upsert(entry);
    if changed {
        let snapshot = ctx.state.network.presence.write().await.snapshot_json();
        ctx.broadcaster.broadcast("presence", snapshot).await;
    }
    Ok(json!({"ok": true}))
}

/// Read-only query: return the current presence list.
pub async fn handle_presence_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let snapshot = ctx.state.network.presence.write().await.snapshot_json();
    Ok(snapshot)
}

pub async fn handle_system_event(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let event_type = params
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("system");
    ctx.broadcaster
        .broadcast(
            "system-event",
            json!({
                "type": event_type,
                "data": params.get("data").cloned().unwrap_or(Value::Null),
            }),
        )
        .await;
    Ok(json!({"ok": true}))
}
