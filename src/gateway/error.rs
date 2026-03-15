use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
#[allow(dead_code)]
pub struct ApiErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub timestamp: String,
}

/// A structured API error that renders as JSON with status code.
#[allow(dead_code)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

#[allow(dead_code)]
impl ApiError {
    pub fn not_found(resource: &str, id: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: format!("{} '{}' not found", resource, id),
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // The request_id is already set by the request_tracing_middleware
        // as a response header (X-Request-Id). We also include it in the
        // JSON body if we can extract it from the current tracing span.
        let request_id = extract_request_id_from_span();

        let body = ApiErrorResponse {
            error: self.message,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        (self.status, Json(body)).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

/// Attempt to extract request_id from the current tracing span's recorded fields.
///
/// This works by looking at the current span's name and checking if it is
/// our `http_request` span, then reading the field value. Since tracing
/// doesn't provide direct field access after creation, we use a visitor
/// pattern on the span extensions (set by our MemoryLogLayer).
#[allow(dead_code)]
fn extract_request_id_from_span() -> Option<String> {
    // The span extensions approach requires LookupSpan access which isn't
    // available in a non-Layer context. Instead, we'll rely on the
    // X-Request-Id response header set by the middleware. For JSON error
    // bodies, we can extract it from span metadata at a best-effort level.
    //
    // A pragmatic approach: use a task-local or read the span's string repr.
    // For now, return None — the X-Request-Id header is always set by middleware.
    None
}
