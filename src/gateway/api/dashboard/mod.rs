mod channels;
mod config;
mod infrastructure;
mod schedules;
mod sessions;
mod skills;
mod stats;

use std::path::Path;

use axum::http::StatusCode;
use axum::Router;
use serde::Serialize;

use crate::gateway::state::AppState;

// ---------------------------------------------------------------------------
// Merged router
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(stats::routes())
        .merge(sessions::routes())
        .merge(schedules::routes())
        .merge(config::routes())
        .merge(channels::routes())
        .merge(skills::routes())
        .merge(infrastructure::routes())
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(super) struct OkResponse {
    pub ok: bool,
}

#[derive(Serialize)]
pub(super) struct ToggleResponse {
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub(super) fn config_file_path() -> String {
    if Path::new("synapse.toml").exists() {
        "synapse.toml".to_string()
    } else {
        "synapse.toml.example".to_string()
    }
}

pub(super) async fn read_config_file() -> Result<(String, String), (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read config: {}", e),
        )
    })?;
    Ok((path, content))
}

pub(super) fn count_bot_channels(config: &crate::config::SynapseConfig) -> usize {
    let checks: &[bool] = &[
        !config.lark.is_empty(),
        !config.slack.is_empty(),
        !config.telegram.is_empty(),
        !config.discord.is_empty(),
        !config.dingtalk.is_empty(),
        !config.mattermost.is_empty(),
        !config.matrix.is_empty(),
        !config.whatsapp.is_empty(),
        !config.teams.is_empty(),
        !config.signal.is_empty(),
        !config.wechat.is_empty(),
        !config.imessage.is_empty(),
        !config.line.is_empty(),
        !config.googlechat.is_empty(),
        !config.irc.is_empty(),
        !config.webchat.is_empty(),
        !config.twitch.is_empty(),
        !config.nostr.is_empty(),
        !config.nextcloud.is_empty(),
        !config.synology.is_empty(),
        !config.tlon.is_empty(),
        !config.zalo.is_empty(),
    ];
    checks.iter().filter(|&&v| v).count()
}

pub(super) fn parse_system_time_string(s: &str) -> String {
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

pub(super) fn sanitize_workspace_filename(filename: &str) -> Result<(), (StatusCode, String)> {
    if filename.is_empty() || filename.len() > 64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename must be 1-64 characters".to_string(),
        ));
    }
    if !filename.ends_with(".md") {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename must end with .md".to_string(),
        ));
    }
    if filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid characters in filename".to_string(),
        ));
    }
    if !filename
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename may only contain [a-zA-Z0-9._-]".to_string(),
        ));
    }
    Ok(())
}
