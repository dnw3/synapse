use std::path::PathBuf;

use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use super::{sanitize_workspace_filename, OkResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/workspace", get(get_workspace_files))
        .route("/dashboard/workspace/{filename}", get(get_workspace_file))
        .route("/dashboard/workspace/{filename}", put(put_workspace_file))
        .route(
            "/dashboard/workspace/{filename}",
            post(create_workspace_file),
        )
        .route(
            "/dashboard/workspace/{filename}",
            delete(delete_workspace_file),
        )
        .route(
            "/dashboard/workspace/{filename}/reset",
            post(reset_workspace_file),
        )
        .route("/dashboard/identity", get(get_identity))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_dir(config: &crate::config::SynapseConfig, agent: Option<&str>) -> PathBuf {
    config.workspace_dir_for_agent(agent)
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/workspace
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WorkspaceQuery {
    agent: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceFileEntry {
    filename: String,
    description: String,
    category: String,
    icon: String,
    exists: bool,
    size_bytes: Option<u64>,
    modified: Option<String>,
    preview: Option<String>,
    is_template: bool,
}

async fn get_workspace_files(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<WorkspaceFileEntry>> {
    use crate::agent::templates::WORKSPACE_TEMPLATES;

    let cwd = workspace_dir(&state.config, query.agent.as_deref());
    let mut entries = Vec::new();

    for tmpl in WORKSPACE_TEMPLATES {
        let path = cwd.join(tmpl.filename);
        let (exists, size_bytes, modified, preview) = if path.exists() {
            let meta = tokio::fs::metadata(&path).await.ok();
            let size = meta.as_ref().map(|m| m.len());
            let mod_time = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });
            let preview = tokio::fs::read_to_string(&path).await.ok().map(|c| {
                let trimmed = c.trim();
                if trimmed.len() > 200 {
                    format!("{}...", &trimmed[..200])
                } else {
                    trimmed.to_string()
                }
            });
            (true, size, mod_time, preview)
        } else {
            (false, None, None, None)
        };

        entries.push(WorkspaceFileEntry {
            filename: tmpl.filename.to_string(),
            description: tmpl.description.to_string(),
            category: tmpl.category.to_string(),
            icon: tmpl.icon.to_string(),
            exists,
            size_bytes,
            modified,
            preview,
            is_template: true,
        });
    }

    if let Ok(mut dir) = tokio::fs::read_dir(&cwd).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            if WORKSPACE_TEMPLATES.iter().any(|t| t.filename == name) {
                continue;
            }
            if name == "README.md" || name == "CHANGELOG.md" || name == "LICENSE.md" {
                continue;
            }
            let path = cwd.join(&name);
            let meta = tokio::fs::metadata(&path).await.ok();
            let size = meta.as_ref().map(|m| m.len());
            let mod_time = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });
            let preview = tokio::fs::read_to_string(&path).await.ok().map(|c| {
                let trimmed = c.trim();
                if trimmed.len() > 200 {
                    format!("{}...", &trimmed[..200])
                } else {
                    trimmed.to_string()
                }
            });
            entries.push(WorkspaceFileEntry {
                filename: name,
                description: "Custom workspace file".to_string(),
                category: "custom".to_string(),
                icon: "file-text".to_string(),
                exists: true,
                size_bytes: size,
                modified: mod_time,
                preview,
                is_template: false,
            });
        }
    }

    Json(entries)
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/workspace/{filename}
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct WorkspaceFileContent {
    filename: String,
    content: String,
    is_template: bool,
}

async fn get_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<WorkspaceFileContent>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    let content = tokio::fs::read_to_string(&path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("file '{}' not found", filename),
        )
    })?;
    let is_template = crate::agent::templates::find_template(&filename).is_some();
    Ok(Json(WorkspaceFileContent {
        filename,
        content,
        is_template,
    }))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/workspace/{filename}
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WorkspaceFileBody {
    content: String,
}

async fn put_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("file '{}' not found \u{2014} use POST to create", filename),
        ));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!(file = %filename, "workspace file saved");

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/workspace/{filename}
// ---------------------------------------------------------------------------

async fn create_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if path.exists() {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "file '{}' already exists \u{2014} use PUT to update",
                filename
            ),
        ));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// DELETE /api/dashboard/workspace/{filename}
// ---------------------------------------------------------------------------

async fn delete_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("file '{}' not found", filename),
        ));
    }
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("delete: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/workspace/{filename}/reset
// ---------------------------------------------------------------------------

async fn reset_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let default = crate::agent::workspace::default_content_for(&filename).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no default template for '{}'", filename),
        )
    })?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    tokio::fs::write(&path, default)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/identity
// ---------------------------------------------------------------------------

async fn get_identity(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<crate::agent::workspace::IdentityInfo> {
    let path = workspace_dir(&state.config, query.agent.as_deref()).join("IDENTITY.md");
    let info = match tokio::fs::read_to_string(&path).await {
        Ok(content) => crate::agent::workspace::parse_identity(&content),
        Err(_) => crate::agent::workspace::IdentityInfo::default(),
    };
    Json(info)
}
