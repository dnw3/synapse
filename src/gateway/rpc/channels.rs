//! RPC handlers for channel (bot platform) management.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

fn config_file_path() -> String {
    if std::path::Path::new("synapse.toml").exists() {
        "synapse.toml".to_string()
    } else {
        "synapse.toml.example".to_string()
    }
}

fn extract_channel_config(toml_val: &toml::Value, channel_name: &str) -> HashMap<String, String> {
    let table = toml_val
        .as_table()
        .and_then(|root| root.get(channel_name))
        .and_then(|v| v.as_table());
    let Some(table) = table else {
        return HashMap::new();
    };
    table
        .iter()
        .filter_map(|(k, v)| {
            let s = match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Boolean(b) => b.to_string(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                _ => return None,
            };
            Some((k.clone(), s))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// channels.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let config = &ctx.state.config;

    let toml_val: toml::Value = {
        let path = config_file_path();
        tokio::fs::read_to_string(&path)
            .await
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or(toml::Value::Table(Default::default()))
    };

    let resolve_enabled = |name: &str, startup_exists: bool| -> bool {
        toml_val
            .get("channel_overrides")
            .and_then(|o| o.get(name))
            .and_then(|c| c.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(startup_exists)
    };

    let channels = vec![
        ("lark", config.lark.is_some()),
        ("slack", config.slack.is_some()),
        ("telegram", config.telegram.is_some()),
        ("discord", config.discord.is_some()),
        ("dingtalk", config.dingtalk.is_some()),
        ("mattermost", config.mattermost.is_some()),
        ("matrix", config.matrix.is_some()),
        ("whatsapp", config.whatsapp.is_some()),
        ("teams", config.teams.is_some()),
        ("signal", config.signal.is_some()),
        ("wechat", config.wechat.is_some()),
        ("imessage", config.imessage.is_some()),
        ("line", config.line.is_some()),
        ("googlechat", config.googlechat.is_some()),
        ("irc", config.irc.is_some()),
        ("webchat", config.webchat.is_some()),
        ("twitch", config.twitch.is_some()),
        ("nostr", config.nostr.is_some()),
        ("nextcloud", config.nextcloud.is_some()),
        ("synology", config.synology.is_some()),
        ("tlon", config.tlon.is_some()),
        ("zalo", config.zalo.is_some()),
    ];

    let result: Vec<Value> = channels
        .into_iter()
        .map(|(name, startup_exists)| {
            json!({
                "name": name,
                "enabled": resolve_enabled(name, startup_exists),
                "config": extract_channel_config(&toml_val, name),
            })
        })
        .collect();

    Ok(json!(result))
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
