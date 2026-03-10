use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use tracing;

use crate::gateway::state::AppState;

#[derive(Deserialize)]
pub struct PathQuery {
    pub path: String,
}

#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct WriteBody {
    pub content: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/files", get(list_files))
        .route("/files/content", get(read_file))
        .route("/files/content", put(write_file))
        .route("/files", delete(delete_file))
}

async fn list_files(
    Query(query): Query<PathQuery>,
) -> Result<Json<Vec<FileEntry>>, (StatusCode, String)> {
    let path = sanitize_path(&query.path)?;

    let mut entries = Vec::new();
    let mut rd = tokio::fs::read_dir(&path).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            format!("cannot read directory: {}", e),
        )
    })?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let meta = entry.metadata().await.ok();
        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
            size: meta.map(|m| m.len()),
        });
    }

    entries.sort_by(|a, b| {
        // Directories first, then by name
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });

    tracing::debug!("directory listed");

    Ok(Json(entries))
}

async fn read_file(
    Query(query): Query<PathQuery>,
) -> Result<Json<FileContent>, (StatusCode, String)> {
    let path = sanitize_path(&query.path)?;

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("cannot read file: {}", e)))?;

    Ok(Json(FileContent {
        path: query.path,
        content,
    }))
}

async fn write_file(
    Query(query): Query<PathQuery>,
    Json(body): Json<WriteBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let path = sanitize_path(&query.path)?;

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    tokio::fs::write(&path, &body.content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("cannot write file: {}", e),
        )
    })?;

    Ok(StatusCode::OK)
}

async fn delete_file(Query(query): Query<PathQuery>) -> Result<StatusCode, (StatusCode, String)> {
    let path = sanitize_path(&query.path)?;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("file not found: {}", e)))?;

    if meta.is_dir() {
        tokio::fs::remove_dir_all(&path).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("cannot delete: {}", e),
            )
        })?;
    } else {
        tokio::fs::remove_file(&path).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("cannot delete: {}", e),
            )
        })?;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Basic path sanitization — reject path traversal.
fn sanitize_path(path: &str) -> Result<String, (StatusCode, String)> {
    if path.contains("..") {
        return Err((
            StatusCode::BAD_REQUEST,
            "path traversal not allowed".to_string(),
        ));
    }
    // Resolve relative to cwd
    let resolved = if path.starts_with('/') {
        path.to_string()
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        cwd.join(path).to_string_lossy().to_string()
    };
    Ok(resolved)
}
