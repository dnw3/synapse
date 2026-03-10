//! Web UI authentication — password login with JWT sessions.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{middleware, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::state::AppState;
use crate::config::AuthConfig;

/// Simple JWT-like token (HMAC-SHA256 would be ideal but we keep deps minimal).
///
/// For production, replace with a proper JWT library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct SessionToken {
    issued_at: u64,
    expires_at: u64,
    nonce: String,
}

/// Auth state shared across handlers.
pub struct AuthState {
    pub config: AuthConfig,
    /// Active session tokens (in-memory; cleared on restart).
    pub sessions: Arc<RwLock<Vec<String>>>,
}

impl AuthState {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Verify a password against the stored hash.
    ///
    /// Supports argon2 hashes (prefix `$argon2`) and falls back to legacy
    /// simple hash for backward compatibility.
    pub fn verify_password(&self, password: &str) -> bool {
        match &self.config.password_hash {
            Some(hash) => {
                if hash.starts_with("$argon2") {
                    // Argon2 verification
                    use argon2::{Argon2, PasswordVerifier};
                    use password_hash::PasswordHash;
                    match PasswordHash::new(hash) {
                        Ok(parsed) => Argon2::default()
                            .verify_password(password.as_bytes(), &parsed)
                            .is_ok(),
                        Err(_) => false,
                    }
                } else {
                    // Legacy fallback: simple hash comparison
                    let input_hash = simple_hash(password);
                    constant_time_eq(hash.as_bytes(), input_hash.as_bytes())
                }
            }
            None => false, // No password set
        }
    }

    /// Hash a password using argon2 for storage.
    #[allow(dead_code)]
    pub fn hash_password(password: &str) -> Result<String, String> {
        use argon2::{Argon2, PasswordHasher};
        use password_hash::rand_core::OsRng;
        use password_hash::SaltString;
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| format!("password hashing failed: {}", e))
    }

    /// Create a new session token.
    pub async fn create_session(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let token = format!("synapse_{}_{}", now, uuid::Uuid::new_v4());
        self.sessions.write().await.push(token.clone());
        token
    }

    /// Check if a session token is valid.
    pub async fn is_valid_session(&self, token: &str) -> bool {
        self.sessions.read().await.contains(&token.to_string())
    }
}

/// Simple hash function (NOT cryptographically strong — use argon2 in production).
fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Constant-time string comparison.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// API handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginRequest {
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    expires_in: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// POST /api/auth/login — authenticate and receive a session token.
pub async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let auth = match &state.auth {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Authentication not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !auth.verify_password(&req.password) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid password".to_string(),
            }),
        )
            .into_response();
    }

    let token = auth.create_session().await;
    (
        StatusCode::OK,
        Json(LoginResponse {
            token,
            expires_in: auth.config.session_duration,
        }),
    )
        .into_response()
}

/// GET /api/auth/status — check if auth is enabled and if the request is authenticated.
pub async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct AuthStatus {
        auth_enabled: bool,
        authenticated: bool,
    }

    let auth_enabled = state
        .auth
        .as_ref()
        .map(|a| a.config.enabled)
        .unwrap_or(false);

    Json(AuthStatus {
        auth_enabled,
        authenticated: !auth_enabled, // If auth disabled, everyone is "authenticated"
    })
}

/// Create the auth API router.
pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", axum::routing::post(login_handler))
        .route("/api/auth/status", axum::routing::get(status_handler))
}

/// Middleware function that checks for a valid auth token.
pub async fn require_auth(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let auth = match &state.auth {
        Some(auth) if auth.config.enabled => auth,
        _ => return next.run(request).await.into_response(), // Auth disabled, pass through
    };

    // Check Authorization header
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if auth.is_valid_session(token).await {
        next.run(request).await.into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Authentication required".to_string(),
            }),
        )
            .into_response()
    }
}
