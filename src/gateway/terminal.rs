//! Terminal WebSocket backend — provides a shell session via WebSocket.

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/terminal", get(terminal_handler))
}

async fn terminal_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_terminal)
}

async fn handle_terminal(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn a shell process
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "/bin/bash"
    };

    let mut child = match Command::new(shell)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = sender
                .send(WsMessage::Text(
                    format!("Failed to spawn shell: {}\r\n", e).into(),
                ))
                .await;
            return;
        }
    };

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    // Forward stdout to WebSocket
    let send_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    if sender.send(WsMessage::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Forward WebSocket input to stdin
    while let Some(Ok(msg)) = receiver.next().await {
        if let WsMessage::Text(text) = msg {
            if stdin.write_all(text.as_bytes()).await.is_err() {
                break;
            }
        }
    }

    drop(stdin);
    send_task.abort();
    let _ = child.kill().await;
}
