use axum::body::Body;
use axum::http::{HeaderValue, Request, Response};
use axum::middleware::Next;

use crate::logging::generate_request_id;

/// Validate that a request ID is reasonable: alphanumeric, dashes, underscores only, max 64 chars.
fn is_valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Axum middleware that:
/// 1. Reads `X-Request-Id` header from request (or generates a new one)
/// 2. Validates the incoming value (alphanumeric + dashes/underscores, max 64 chars)
/// 3. Creates a tracing span with request_id, method, path
/// 4. Adds `X-Request-Id` to response headers
/// 5. Logs request completion with status and duration
pub async fn request_tracing_middleware(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| is_valid_request_id(v))
        .map(String::from)
        .unwrap_or_else(generate_request_id);

    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = std::time::Instant::now();

    // Skip noisy internal polling endpoints from log tracing
    let is_internal = path.starts_with("/api/logs")
        || path.starts_with("/api/dashboard/")
        || path == "/api/dashboard"
        || path.starts_with("/uploads/");

    if is_internal {
        // Still set X-Request-Id header but don't log
        let mut response = next.run(request).await;
        if let Ok(val) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert("x-request-id", val);
        }
        return response;
    }

    // Create a tracing span that carries the request_id.
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        path = %path,
    );

    let mut response = {
        use tracing::Instrument;
        async move { next.run(request).await }
            .instrument(span.clone())
            .await
    };

    let duration_ms = start.elapsed().as_millis();
    let status = response.status().as_u16();

    {
        let _guard = span.enter();
        tracing::info!(
            status = status,
            duration_ms = duration_ms as u64,
            "request completed"
        );
    }

    // Add X-Request-Id to response headers
    if let Ok(val) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}
