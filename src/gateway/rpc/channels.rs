//! RPC handlers for channel (bot platform) management.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

fn system_time_to_secs(t: SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn opt_time_to_value(t: Option<SystemTime>) -> Value {
    match t {
        Some(t) => json!(system_time_to_secs(t)),
        None => Value::Null,
    }
}

#[derive(Deserialize, Default)]
struct StatusParams {
    #[serde(default)]
    probe: bool,
}

// ---------------------------------------------------------------------------
// channels.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let params: StatusParams = serde_json::from_value(params).unwrap_or_default();
    let config = &ctx.state.core.config;

    // Collect live snapshots from the channel manager
    let snapshots = ctx.state.channel.channel_manager.snapshot_all().await;

    // Group snapshots by channel name
    let mut by_channel: HashMap<String, Vec<Value>> = HashMap::new();
    for snap in &snapshots {
        let disconnect_val = snap.last_disconnect.as_ref().map(|d| {
            json!({
                "at": system_time_to_secs(d.at),
                "error": d.error,
            })
        });

        let entry = json!({
            "account_id": snap.account_id,
            "state": snap.state.to_string(),
            "running": snap.running,
            "busy": snap.busy,
            "active_runs": snap.active_runs,
            "connected_at": opt_time_to_value(snap.connected_at),
            "last_event_at": opt_time_to_value(snap.last_event_at),
            "last_inbound_at": opt_time_to_value(snap.last_inbound_at),
            "last_outbound_at": opt_time_to_value(snap.last_outbound_at),
            "last_error": snap.last_error,
            "reconnect_count": snap.reconnect_count,
            "mode": snap.mode,
            "last_disconnect": disconnect_val,
        });

        by_channel
            .entry(snap.channel.clone())
            .or_default()
            .push(entry);
    }

    // All configured channel names and their account counts
    let configured_channels: Vec<(&str, usize)> = config
        .channels
        .iter()
        .map(|(name, accounts)| (name.as_str(), accounts.len()))
        .collect();

    // Build the channels map: include all channels that are either configured or running
    let mut channels: HashMap<String, Value> = HashMap::new();
    for (name, configured_count) in &configured_channels {
        let accounts = by_channel.remove(*name).unwrap_or_default();
        channels.insert(
            name.to_string(),
            json!({
                "configured": *configured_count,
                "accounts": accounts,
            }),
        );
    }

    // Include any channels that are running but not in the known list
    for (name, accounts) in by_channel {
        channels.insert(
            name.clone(),
            json!({
                "configured": 0,
                "accounts": accounts,
            }),
        );
    }

    let ts = system_time_to_secs(SystemTime::now());

    // Optionally run probes
    let probe_results = if params.probe {
        let probes = ctx.state.channel.channel_manager.run_probes().await;
        let results: Vec<Value> = probes
            .into_iter()
            .map(|(channel, account_id, result)| {
                let (ok, error) = match result {
                    Ok(()) => (true, Value::Null),
                    Err(e) => (false, json!(e)),
                };
                json!({
                    "channel": channel,
                    "account_id": account_id,
                    "ok": ok,
                    "error": error,
                })
            })
            .collect();
        Some(results)
    } else {
        None
    };

    let mut result = json!({
        "ts": ts,
        "channels": channels,
    });

    if let Some(probes) = probe_results {
        result
            .as_object_mut()
            .unwrap()
            .insert("probe_results".to_string(), json!(probes));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// channels.logout
// ---------------------------------------------------------------------------

pub async fn handle_logout(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    tracing::info!(channel = %name, "channel logout requested via RPC");

    // Placeholder — actual channel disconnection requires runtime adapter handles
    Ok(json!({ "ok": true, "channel": name }))
}
