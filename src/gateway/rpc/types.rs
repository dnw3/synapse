//! Protocol v3 frame types for the RPC transport layer.
//!
//! These types define the wire format for client↔server communication
//! over WebSocket, aligned with the OpenClaw protocol v3 spec.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 3;

// ---------------------------------------------------------------------------
// Client → Server frames
// ---------------------------------------------------------------------------

/// A frame sent from the client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    /// An RPC request expecting a response.
    Request {
        /// Unique request identifier (echoed in the response).
        id: String,
        /// Method name, e.g. "health", "agent", "chat.send".
        method: String,
        /// Method-specific parameters (may be `{}` or omitted).
        #[serde(default)]
        params: Value,
    },
}

// ---------------------------------------------------------------------------
// Server → Client frames
// ---------------------------------------------------------------------------

/// A frame sent from the server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    /// A response to a client request.
    Response {
        /// Echoed request id.
        id: String,
        /// Whether the request succeeded.
        ok: bool,
        /// Successful result payload.
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<Value>,
        /// Error details on failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<RpcError>,
    },
    /// A server-initiated event (push notification).
    Event {
        /// Event name, e.g. "agent.message.delta".
        event: String,
        /// Event-specific payload.
        payload: Value,
        /// Monotonically increasing sequence number per connection.
        seq: u64,
        /// Current server state version snapshot.
        #[serde(skip_serializing_if = "Option::is_none")]
        state_version: Option<StateVersion>,
    },
}

impl ServerFrame {
    /// Create a successful response frame.
    pub fn ok(id: impl Into<String>, payload: Value) -> Self {
        Self::Response {
            id: id.into(),
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    /// Create an error response frame.
    pub fn err(id: impl Into<String>, error: RpcError) -> Self {
        Self::Response {
            id: id.into(),
            ok: false,
            payload: None,
            error: Some(error),
        }
    }

    /// Create a server-push event frame.
    pub fn event(event: impl Into<String>, payload: Value, seq: u64) -> Self {
        Self::Event {
            event: event.into(),
            payload,
            seq,
            state_version: None,
        }
    }
}

// ---------------------------------------------------------------------------
// RPC Error
// ---------------------------------------------------------------------------

/// Structured error returned in a response frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Machine-readable error code (loosely follows JSON-RPC / HTTP conventions).
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// Whether the client should retry the request.
    #[serde(default)]
    pub retryable: bool,
    /// Suggested retry delay in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl RpcError {
    /// 400 — malformed request.
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            code: 400,
            message: msg.into(),
            details: None,
            retryable: false,
            retry_after_ms: None,
        }
    }

    /// 404 — resource not found.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: 404,
            message: msg.into(),
            details: None,
            retryable: false,
            retry_after_ms: None,
        }
    }

    /// 403 — insufficient permissions.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            code: 403,
            message: msg.into(),
            details: None,
            retryable: false,
            retry_after_ms: None,
        }
    }

    /// 500 — internal server error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: 500,
            message: msg.into(),
            details: None,
            retryable: true,
            retry_after_ms: Some(1000),
        }
    }

    /// -32601 — method not found (JSON-RPC convention).
    pub fn method_not_found(method: impl Into<String>) -> Self {
        let m = method.into();
        Self {
            code: -32601,
            message: format!("Method not found: {m}"),
            details: None,
            retryable: false,
            retry_after_ms: None,
        }
    }
}

// ---------------------------------------------------------------------------
// State versioning
// ---------------------------------------------------------------------------

/// Monotonic version counters for different state domains.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateVersion {
    pub presence: u64,
    pub health: u64,
}

// ---------------------------------------------------------------------------
// Connect / Hello handshake types
// ---------------------------------------------------------------------------

/// Parameters sent by the client in a `connect` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectParams {
    /// Minimum protocol version the client supports.
    #[serde(default = "default_protocol_version")]
    pub min_protocol: u32,
    /// Maximum protocol version the client supports.
    #[serde(default = "default_protocol_version")]
    pub max_protocol: u32,
    /// Client identification.
    #[serde(default)]
    pub client: ClientInfo,
    /// Requested capabilities / feature flags.
    #[serde(default)]
    pub caps: Vec<String>,
    /// Slash-commands the client knows about.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Requested role for the connection.
    #[serde(default)]
    pub role: Option<String>,
    /// Requested permission scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Additional permissions.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Client's PATH environment.
    #[serde(default)]
    pub path_env: Option<String>,
    /// Authentication parameters.
    #[serde(default)]
    pub auth: Option<AuthParams>,
    /// Device-level authentication (for node connections).
    #[serde(default)]
    pub device: Option<DeviceAuth>,
    /// Client locale, e.g. "en-US".
    #[serde(default)]
    pub locale: Option<String>,
    /// Client user-agent string.
    #[serde(default)]
    pub user_agent: Option<String>,
}

fn default_protocol_version() -> u32 {
    PROTOCOL_VERSION
}

/// Client identification included in a connect request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Unique client id (e.g. "vscode", "cli").
    #[serde(default)]
    pub id: String,
    /// Human-readable display name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Client version string.
    #[serde(default)]
    pub version: Option<String>,
    /// Platform, e.g. "darwin", "linux", "win32".
    #[serde(default)]
    pub platform: Option<String>,
    /// Device family, e.g. "desktop", "mobile".
    #[serde(default)]
    pub device_family: Option<String>,
    /// Model identifier for the underlying LLM.
    #[serde(default)]
    pub model_identifier: Option<String>,
    /// Interaction mode, e.g. "chat", "agent".
    #[serde(default)]
    pub mode: Option<String>,
    /// Unique instance id for this connection.
    #[serde(default)]
    pub instance_id: Option<String>,
}

/// Token / password-based authentication parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthParams {
    /// Bearer token.
    #[serde(default)]
    pub token: Option<String>,
    /// Password (for password-based auth).
    #[serde(default)]
    pub password: Option<String>,
}

/// Device-level authentication (public-key signature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuth {
    /// Device identifier.
    pub id: String,
    /// Public key (PEM or base64-encoded).
    pub public_key: String,
    /// Signature over a challenge.
    pub signature: String,
    /// Timestamp when the signature was created.
    pub signed_at: String,
    /// One-time nonce to prevent replay attacks.
    pub nonce: String,
}

// ---------------------------------------------------------------------------
// Hello response (connect result)
// ---------------------------------------------------------------------------

/// Successful response to a `connect` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloOk {
    /// Negotiated protocol version.
    pub protocol: u32,
    /// Server identification.
    pub server: ServerInfo,
    /// Supported features.
    pub features: FeatureInfo,
    /// Initial state snapshot.
    pub snapshot: SnapshotInfo,
    /// Authentication result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_result: Option<AuthResult>,
    /// Applicable policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<Value>,
}

/// Server identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server version string.
    pub version: String,
    /// Unique connection id assigned by the server.
    pub conn_id: String,
}

/// Supported features advertised by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    /// Available RPC method names.
    pub methods: Vec<String>,
    /// Available server-push event names.
    pub events: Vec<String>,
}

/// Initial state snapshot included in the hello response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// Presence information (who is connected).
    #[serde(default)]
    pub presence: Value,
    /// Current health status.
    #[serde(default)]
    pub health: Value,
    /// State version at snapshot time.
    pub state_version: StateVersion,
}

/// Authentication result returned in hello.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    /// Whether authentication succeeded.
    pub authenticated: bool,
    /// Granted role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Granted scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}
