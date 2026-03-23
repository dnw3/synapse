mod agent;
mod auth;
mod connection;
mod streaming_output;
mod types;
mod utils;

#[allow(unused_imports)]
pub use streaming_output::WsStreamingOutput;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::{routing::get, Router};
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::gateway::rpc::ClientFrame;
use crate::gateway::state::AppState;

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/ws/gateway", get(ws_handler))
        .with_state(state)
}

/// Unified WebSocket handler — single `/ws` endpoint, no session in URL.
///
/// Each `chat.send` request carries a `sessionKey` in its params, allowing
/// a single connection to interact with multiple sessions.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Unique identifier for this WebSocket connection (used as lock holder).
    let conn_id = Uuid::new_v4().to_string();

    tracing::info!(%conn_id, "websocket connected");

    // --- Protocol v3: send connect.challenge before anything else ---
    let nonce = Uuid::new_v4().to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let challenge = serde_json::json!({
        "type": "event",
        "event": "connect.challenge",
        "payload": { "nonce": nonce, "ts": ts }
    });
    let _ = sender
        .send(WsMessage::Text(
            serde_json::to_string(&challenge).unwrap().into(),
        ))
        .await;

    // --- Wait for first client frame to detect protocol version ---
    let first_msg = match receiver.next().await {
        Some(Ok(WsMessage::Text(text))) => text.to_string(),
        _ => {
            tracing::info!(%conn_id, "client disconnected before sending first frame");
            return;
        }
    };

    // Try to parse as a v3 ClientFrame ({"type":"request","id":"...","method":"connect",...})
    let is_v3 = serde_json::from_str::<ClientFrame>(&first_msg)
        .ok()
        .map(|frame| matches!(&frame, ClientFrame::Request { method, .. } if method == "connect"))
        .unwrap_or(false);

    if is_v3 {
        connection::handle_v3_connection(sender, receiver, state, conn_id, first_msg).await;
    } else {
        let err = serde_json::json!({
            "type": "event", "event": "error",
            "payload": { "message": "Unsupported protocol. Please use v3 connect handshake." }
        });
        let _ = sender
            .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
            .await;
        let _ = sender.close().await;
    }
}
