use std::collections::HashSet;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::stream::SplitSink;
use futures::SinkExt;

use crate::gateway::rpc::{
    AuthResult, ConnectParams, FeatureInfo, HelloOk, Role, RpcError, ServerFrame, ServerInfo,
    SnapshotInfo, StateVersion, GATEWAY_EVENTS, PROTOCOL_VERSION,
};
use crate::gateway::state::AppState;

/// Result of a successful WebSocket authentication.
pub(super) struct WsAuthResult {
    pub role: Role,
    pub scopes: HashSet<String>,
}

/// Validate credentials from a v3 connect request.
///
/// Returns `Some(WsAuthResult)` on success, or `None` after sending an error
/// frame back to the client.
pub(super) async fn authenticate(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    connect_params: &ConnectParams,
    req_id: &str,
    conn_id: &str,
) -> Option<WsAuthResult> {
    match &state.core.auth {
        Some(auth_state) if auth_state.config.enabled => {
            // Auth is enabled — validate token or password
            let authenticated = if let Some(ref auth_params) = connect_params.auth {
                if let Some(ref token) = auth_params.token {
                    auth_state.is_valid_session(token).await
                } else if let Some(ref password) = auth_params.password {
                    auth_state.verify_password(password)
                } else {
                    false
                }
            } else {
                false
            };

            if !authenticated {
                let err_frame =
                    ServerFrame::err(req_id, RpcError::forbidden("Authentication failed"));
                let _ = sender
                    .send(WsMessage::Text(
                        serde_json::to_string(&err_frame).unwrap().into(),
                    ))
                    .await;
                tracing::warn!(%conn_id, "v3 connect rejected: auth failed");
                return None;
            }

            // Determine role and scopes from the connect request
            let role = match connect_params.role.as_deref() {
                Some("node") => Role::Node,
                _ => Role::Operator,
            };
            let mut granted_scopes: HashSet<String> =
                connect_params.scopes.iter().cloned().collect();
            // Default to operator.admin for authenticated operators
            if role == Role::Operator && granted_scopes.is_empty() {
                granted_scopes.insert("operator.admin".to_string());
            }
            Some(WsAuthResult {
                role,
                scopes: granted_scopes,
            })
        }
        _ => {
            // Auth disabled — grant full access
            let role = match connect_params.role.as_deref() {
                Some("node") => Role::Node,
                _ => Role::Operator,
            };
            let mut scopes: HashSet<String> = connect_params.scopes.iter().cloned().collect();
            if scopes.is_empty() {
                scopes.insert("operator.admin".to_string());
            }
            Some(WsAuthResult { role, scopes })
        }
    }
}

/// Build and send the hello-ok response frame after successful authentication.
pub(super) async fn send_hello_ok(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    req_id: &str,
    conn_id: &str,
    role: Role,
    scopes: &HashSet<String>,
) {
    let hello_ok = HelloOk {
        protocol: PROTOCOL_VERSION,
        server: ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            conn_id: conn_id.to_string(),
        },
        features: FeatureInfo {
            methods: state.network.rpc_router.method_names(),
            events: GATEWAY_EVENTS.iter().map(|s| s.to_string()).collect(),
        },
        snapshot: {
            let mut pstore = state.network.presence.write().await;
            let pver = pstore.version();
            let psnap = pstore.snapshot_json();
            drop(pstore);
            SnapshotInfo {
                presence: psnap,
                health: serde_json::json!({"status": "ok"}),
                state_version: StateVersion {
                    presence: pver,
                    ..Default::default()
                },
            }
        },
        auth_result: Some(AuthResult {
            authenticated: true,
            role: Some(format!("{:?}", role).to_lowercase()),
            scopes: scopes.iter().cloned().collect(),
        }),
        policy: None,
    };

    let hello_frame = ServerFrame::ok(req_id, serde_json::to_value(&hello_ok).unwrap());
    let _ = sender
        .send(WsMessage::Text(
            serde_json::to_string(&hello_frame).unwrap().into(),
        ))
        .await;
}
