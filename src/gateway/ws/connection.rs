use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};

use crate::gateway::rpc::{
    ClientFrame, ConnectParams, RpcContext, RpcError, ServerFrame, PROTOCOL_VERSION,
};
use crate::gateway::state::AppState;
use crate::session::key as session_key;

pub(super) async fn handle_v3_connection(
    mut sender: SplitSink<WebSocket, WsMessage>,
    mut receiver: SplitStream<WebSocket>,
    state: AppState,
    conn_id: String,
    first_msg: String,
) {
    // Parse the connect request (we already know it's a valid ClientFrame::Request with method=="connect")
    let (req_id, connect_params) = match serde_json::from_str::<ClientFrame>(&first_msg) {
        Ok(ClientFrame::Request { id, params, .. }) => {
            let cp: ConnectParams =
                serde_json::from_value(params).unwrap_or_else(|_| ConnectParams {
                    min_protocol: PROTOCOL_VERSION,
                    max_protocol: PROTOCOL_VERSION,
                    client: Default::default(),
                    caps: vec![],
                    commands: vec![],
                    role: None,
                    scopes: vec![],
                    permissions: vec![],
                    path_env: None,
                    auth: None,
                    device: None,
                    locale: None,
                    user_agent: None,
                });
            (id, cp)
        }
        _ => {
            tracing::error!(%conn_id, "v3 connect frame parse failed after detection");
            return;
        }
    };

    // --- Auth validation ---
    let auth_result =
        match super::auth::authenticate(&mut sender, &state, &connect_params, &req_id, &conn_id)
            .await
        {
            Some(result) => result,
            None => return, // auth failed, error already sent
        };
    let role = auth_result.role;
    let scopes = auth_result.scopes;

    // --- Register connection in broadcaster ---
    let mut event_rx = state.network.broadcaster.register(conn_id.clone()).await;

    // --- Build RpcContext ---
    let rpc_ctx = Arc::new(RpcContext {
        state: state.clone(),
        conn_id: conn_id.clone(),
        client: connect_params.client.clone(),
        role,
        scopes: scopes.clone(),
        broadcaster: state.network.broadcaster.clone(),
    });

    // --- Build and send hello-ok response ---
    super::auth::send_hello_ok(&mut sender, &state, &req_id, &conn_id, role, &scopes).await;

    tracing::info!(%conn_id, ?role, "v3 connection established");

    // Event sequence counter for this connection
    let seq = AtomicU64::new(1);

    // Tick timer for keepalive / heartbeat
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Track which session keys this connection has used (for cleanup)
    let mut active_session_keys: HashSet<String> = HashSet::new();

    // --- V3 main loop ---
    loop {
        tokio::select! {
            // Periodic tick event
            _ = tick_interval.tick() => {
                let tick_seq = seq.fetch_add(1, Ordering::Relaxed);
                let tick_frame = ServerFrame::event("tick", serde_json::json!({
                    "ts": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                }), tick_seq);
                let _ = sender
                    .send(WsMessage::Text(
                        serde_json::to_string(&tick_frame).unwrap().into(),
                    ))
                    .await;
            }
            // Client frames from WebSocket
            msg = receiver.next() => {
                let text = match msg {
                    Some(Ok(WsMessage::Text(t))) => t.to_string(),
                    Some(Ok(WsMessage::Close(_))) | None => {
                        tracing::info!(%conn_id, "v3 client disconnected");
                        break;
                    }
                    Some(Ok(_)) => continue, // skip binary/ping/pong
                    Some(Err(e)) => {
                        tracing::warn!(%conn_id, error = %e, "v3 websocket error");
                        break;
                    }
                };

                // Parse as ClientFrame
                let frame = match serde_json::from_str::<ClientFrame>(&text) {
                    Ok(f) => f,
                    Err(e) => {
                        let err_frame = ServerFrame::err(
                            "unknown",
                            RpcError::invalid_request(format!("Invalid frame: {e}")),
                        );
                        let _ = sender
                            .send(WsMessage::Text(
                                serde_json::to_string(&err_frame).unwrap().into(),
                            ))
                            .await;
                        continue;
                    }
                };

                match frame {
                    ClientFrame::Request { id, method, params } => {
                        if method == "agent" || method == "chat.send" {
                            // Extract sessionKey from params (default: "main")
                            let sk = params
                                .get("sessionKey")
                                .or_else(|| params.get("session_key"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("main")
                                .to_string();

                            // Validate the session key
                            if let Err(e) = session_key::validate_request_key(&sk) {
                                let err = ServerFrame::err(
                                    &id,
                                    RpcError::invalid_request(format!("Invalid sessionKey: {e}")),
                                );
                                let _ = sender
                                    .send(WsMessage::Text(
                                        serde_json::to_string(&err).unwrap().into(),
                                    ))
                                    .await;
                                continue;
                            }

                            active_session_keys.insert(sk.clone());

                            // Handle agent execution with per-request session key
                            super::agent::handle_v3_agent(
                                &mut sender,
                                &mut receiver,
                                &sk,
                                &state,
                                &conn_id,
                                &id,
                                &params,
                                &seq,
                            )
                            .await;
                        } else if method == "ping" {
                            let pong = ServerFrame::ok(&id, serde_json::json!({"pong": true}));
                            let _ = sender
                                .send(WsMessage::Text(
                                    serde_json::to_string(&pong).unwrap().into(),
                                ))
                                .await;
                        } else {
                            // Standard RPC dispatch via rpc_router
                            let response = state
                                .network.rpc_router
                                .dispatch(rpc_ctx.clone(), id, &method, params)
                                .await;
                            let _ = sender
                                .send(WsMessage::Text(
                                    serde_json::to_string(&response).unwrap().into(),
                                ))
                                .await;
                        }
                    }
                }
            }
            // Events from Broadcaster
            Some(frame) = event_rx.recv() => {
                let _ = sender
                    .send(WsMessage::Text(
                        serde_json::to_string(&frame).unwrap().into(),
                    ))
                    .await;
            }
        }
    }

    // --- Cleanup ---
    state.network.broadcaster.unregister(&conn_id).await;
    // Release write locks and cancel tokens for all session keys used by this connection
    for sk in &active_session_keys {
        let store_key = session_key::to_store_key("default", sk);
        state.session.cancel_tokens.write().await.remove(&store_key);
        state.session.write_lock.release(&store_key, &conn_id).await;
    }
    tracing::info!(%conn_id, "v3 connection closed");
}
