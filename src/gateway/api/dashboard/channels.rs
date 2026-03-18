use std::collections::HashMap;

use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post, put};
use axum::Router;
use serde::Serialize;

use super::{config_file_path, read_config_file, OkResponse, ToggleResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/channels", get(get_channels))
        .route("/dashboard/channels/{name}/toggle", post(toggle_channel))
        .route("/dashboard/channels/{name}/config", put(put_channel_config))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/channels
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ChannelResponse {
    name: String,
    enabled: bool,
    config: HashMap<String, String>,
}

fn extract_channel_config(toml_val: &toml::Value, channel_name: &str) -> HashMap<String, String> {
    let entry = toml_val.as_table().and_then(|root| root.get(channel_name));
    let table = entry.and_then(|v| match v {
        toml::Value::Table(t) => Some(t),
        toml::Value::Array(arr) => arr.first().and_then(|item| item.as_table()),
        _ => None,
    });
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

async fn get_channels(State(state): State<AppState>) -> Json<Vec<ChannelResponse>> {
    let config = &state.config;

    let toml_val: toml::Value = read_config_file()
        .await
        .ok()
        .and_then(|(_, content)| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()));

    let resolve_enabled = |name: &str, startup_exists: bool| -> bool {
        toml_val
            .get("channel_overrides")
            .and_then(|o| o.get(name))
            .and_then(|c| c.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(startup_exists)
    };

    let channels = vec![
        ("lark", !config.lark.is_empty()),
        ("slack", !config.slack.is_empty()),
        ("telegram", !config.telegram.is_empty()),
        ("discord", !config.discord.is_empty()),
        ("dingtalk", !config.dingtalk.is_empty()),
        ("mattermost", !config.mattermost.is_empty()),
        ("matrix", !config.matrix.is_empty()),
        ("whatsapp", !config.whatsapp.is_empty()),
        ("teams", !config.teams.is_empty()),
        ("signal", !config.signal.is_empty()),
        ("wechat", !config.wechat.is_empty()),
        ("imessage", !config.imessage.is_empty()),
        ("line", !config.line.is_empty()),
        ("googlechat", !config.googlechat.is_empty()),
        ("irc", !config.irc.is_empty()),
        ("webchat", !config.webchat.is_empty()),
        ("twitch", !config.twitch.is_empty()),
        ("nostr", !config.nostr.is_empty()),
        ("nextcloud", !config.nextcloud.is_empty()),
        ("synology", !config.synology.is_empty()),
        ("tlon", !config.tlon.is_empty()),
        ("zalo", !config.zalo.is_empty()),
    ];

    Json(
        channels
            .into_iter()
            .map(|(name, startup_exists)| ChannelResponse {
                config: extract_channel_config(&toml_val, name),
                enabled: resolve_enabled(name, startup_exists),
                name: name.to_string(),
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/channels/{name}/toggle
// ---------------------------------------------------------------------------

async fn toggle_channel(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<ToggleResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    let known_channels = [
        "lark",
        "slack",
        "telegram",
        "discord",
        "dingtalk",
        "mattermost",
        "matrix",
        "whatsapp",
        "teams",
        "signal",
        "wechat",
        "imessage",
        "line",
        "googlechat",
        "irc",
        "webchat",
        "twitch",
        "nostr",
        "nextcloud",
        "synology",
        "tlon",
        "zalo",
    ];
    if !known_channels.contains(&name.as_str()) {
        return Err((StatusCode::NOT_FOUND, format!("unknown channel '{}'", name)));
    }
    let section_exists = doc
        .get(&name)
        .and_then(|v| v.as_table())
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    let current_override = doc
        .get("channel_overrides")
        .and_then(|o| o.get(&name))
        .and_then(|c| c.get("enabled"))
        .and_then(|v| v.as_bool());
    let current_enabled = current_override.unwrap_or(section_exists);
    let new_enabled = !current_enabled;

    let root = doc.as_table_mut().unwrap();
    let overrides = root
        .entry("channel_overrides")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    if let toml::Value::Table(tbl) = overrides {
        let ch_entry = tbl
            .entry(&name)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let toml::Value::Table(ch_tbl) = ch_entry {
            ch_tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    let enabled = new_enabled;
    tracing::info!(channel = %name, enabled, "channel toggled");

    Ok(Json(ToggleResponse {
        enabled: new_enabled,
    }))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/channels/{name}/config
// ---------------------------------------------------------------------------

async fn put_channel_config(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read failed: {}", e),
        )
    })?;

    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse failed: {}", e),
        )
    })?;

    let root = doc.as_table_mut().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "config root is not a table".to_string(),
        )
    })?;

    if !root.contains_key(&name) {
        root.insert(name.clone(), toml::Value::Table(Default::default()));
    }
    let section = root
        .get_mut(&name)
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "channel section is not a table".to_string(),
            )
        })?;

    for (key, value) in body {
        if value.is_empty() {
            section.remove(&key);
        } else {
            section.insert(key, toml::Value::String(value));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize failed: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("write failed: {}", e),
        )
    })?;

    tracing::info!(channel = %name, "channel config saved");

    Ok(Json(OkResponse { ok: true }))
}
