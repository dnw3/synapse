use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use serde::Serialize;
use uuid::Uuid;

use tracing;

use crate::gateway::state::AppState;

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

const ALLOWED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", // images
    "txt", "md", "csv", "json", "xml", "yaml", "yml", "toml", "log", // text
    "pdf", // documents
    "rs", "py", "js", "ts", "go", "java", "c", "cpp", "h", "rb", "sh", // code
];

#[derive(Serialize)]
pub struct UploadResponse {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub url: String,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/upload", post(upload_file))
}

async fn upload_file(
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    let upload_dir = std::path::Path::new("data/uploads");
    if !upload_dir.exists() {
        std::fs::create_dir_all(upload_dir)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create upload dir: {}", e)))?;
    }

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name != "file" {
            continue;
        }

        let original_filename = field
            .file_name()
            .unwrap_or("unknown")
            .to_string();

        let mime_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("failed to read file: {}", e)))?;

        if data.len() > MAX_FILE_SIZE {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("file too large: {} bytes (max {})", data.len(), MAX_FILE_SIZE),
            ));
        }

        // Validate extension
        let ext = std::path::Path::new(&original_filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("file type not allowed: .{}", ext),
            ));
        }

        let file_id = Uuid::new_v4().to_string();
        let stored_name = if ext.is_empty() {
            file_id.clone()
        } else {
            format!("{}.{}", file_id, ext)
        };

        let file_path = upload_dir.join(&stored_name);
        std::fs::write(&file_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to write file: {}", e)))?;

        let size = data.len();
        let name = &original_filename;
        tracing::info!(filename = %name, size, "file uploaded");

        return Ok(Json(UploadResponse {
            id: file_id,
            filename: original_filename,
            mime_type,
            size,
            url: format!("/uploads/{}", stored_name),
        }));
    }

    Err((StatusCode::BAD_REQUEST, "no file field found".to_string()))
}
