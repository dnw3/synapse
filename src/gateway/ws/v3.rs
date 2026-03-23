use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use synaptic::core::RunContext;
use synaptic::deep::StreamingOutputHandle;
use synaptic::events::{Event, EventKind};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::streaming_output::WsStreamingOutput;
use super::types::Attachment;
use crate::gateway::messages::{Attachment as EnvelopeAttachment, InboundMessage};
use crate::gateway::rpc::{
    AuthResult, ClientFrame, ConnectParams, FeatureInfo, HelloOk, Role, RpcContext, RpcError,
    ServerFrame, ServerInfo, SnapshotInfo, StateVersion, GATEWAY_EVENTS, PROTOCOL_VERSION,
};
use crate::gateway::state::AppState;
use crate::session::key as session_key;

/// Unified WebSocket handler — single `/ws` endpoint, no session in URL.
///
/// Each `chat.send` request carries a `sessionKey` in its params, allowing
/// a single connection to interact with multiple sessions.
pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
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
        handle_v3_connection(sender, receiver, state, conn_id, first_msg).await;
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

async fn handle_v3_connection(
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
    let (role, scopes) = match &state.auth {
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
                    ServerFrame::err(&req_id, RpcError::forbidden("Authentication failed"));
                let _ = sender
                    .send(WsMessage::Text(
                        serde_json::to_string(&err_frame).unwrap().into(),
                    ))
                    .await;
                tracing::warn!(%conn_id, "v3 connect rejected: auth failed");
                return;
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
            (role, granted_scopes)
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
            (role, scopes)
        }
    };

    // --- Register connection in broadcaster ---
    let mut event_rx = state.broadcaster.register(conn_id.clone()).await;

    // --- Build RpcContext ---
    let rpc_ctx = Arc::new(RpcContext {
        state: state.clone(),
        conn_id: conn_id.clone(),
        client: connect_params.client.clone(),
        role,
        scopes: scopes.clone(),
        broadcaster: state.broadcaster.clone(),
    });

    // --- Build and send hello-ok response ---
    let hello_ok = HelloOk {
        protocol: PROTOCOL_VERSION,
        server: ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            conn_id: conn_id.clone(),
        },
        features: FeatureInfo {
            methods: state.rpc_router.method_names(),
            events: GATEWAY_EVENTS.iter().map(|s| s.to_string()).collect(),
        },
        snapshot: {
            let mut pstore = state.presence.write().await;
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

    let hello_frame = ServerFrame::ok(&req_id, serde_json::to_value(&hello_ok).unwrap());
    let _ = sender
        .send(WsMessage::Text(
            serde_json::to_string(&hello_frame).unwrap().into(),
        ))
        .await;

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
                            handle_v3_agent(
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
                                .rpc_router
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
    state.broadcaster.unregister(&conn_id).await;
    // Release write locks and cancel tokens for all session keys used by this connection
    for sk in &active_session_keys {
        let store_key = session_key::to_store_key("default", sk);
        state.cancel_tokens.write().await.remove(&store_key);
        state.write_lock.release(&store_key, &conn_id).await;
    }
    tracing::info!(%conn_id, "v3 connection closed");
}

/// Handle an `agent` or `chat.send` RPC request in v3 protocol.
///
/// The `session_key_str` is the client-facing session key (e.g. "main"),
/// extracted from the request params. It is converted to a store key
/// (e.g. "agent:default:main") for internal use.
///
/// Uses the unified `AgentSession::handle_message_streaming_with_context()`
/// pipeline with `WsStreamingOutput` for real-time event forwarding. The
/// `StreamingInterceptor` in the middleware chain handles token streaming
/// automatically via `RunContext`.
#[allow(clippy::too_many_arguments)]
async fn handle_v3_agent(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    receiver: &mut SplitStream<WebSocket>,
    session_key_str: &str,
    state: &AppState,
    conn_id: &str,
    request_id_rpc: &str,
    params: &Value,
    seq: &AtomicU64,
) {
    let request_id = synaptic::logging::generate_request_id();
    let store_key = session_key::to_store_key("default", session_key_str);

    let req_span = tracing::info_span!(
        "ws_v3_request",
        %request_id,
        %conn_id,
        session_key = %session_key_str,
    );
    let _req_guard = req_span.enter();

    // --- Parse content and attachments from params ---
    // Accept both "message" (OpenClaw-aligned) and "content" (legacy) parameter names
    let content = params
        .get("message")
        .or_else(|| params.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ws_attachments: Vec<Attachment> = params
        .get("attachments")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if content.is_empty() && ws_attachments.is_empty() {
        let err = ServerFrame::err(
            request_id_rpc,
            RpcError::invalid_request("Missing 'content' in params"),
        );
        let _ = sender
            .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
            .await;
        return;
    }

    tracing::info!(
        msg_type = "agent",
        content_len = content.len(),
        session_key = %session_key_str,
        "v3 agent request"
    );

    // --- Emit MessageReceived event (fire-and-forget) ---
    {
        let event_bus = state.event_bus.clone();
        let req_id = request_id.clone();
        let sk = session_key_str.to_string();
        tokio::spawn(async move {
            let mut event = Event::new(
                EventKind::MessageReceived,
                serde_json::json!({
                    "request_id": req_id,
                    "session_key": sk,
                    "channel": "web",
                    "protocol": "v3",
                }),
            )
            .with_source("gateway/ws");
            let _ = event_bus.emit(&mut event).await;
        });
    }

    // Helper to send a v3 event frame
    macro_rules! send_event {
        ($event:expr, $payload:expr) => {{
            let s = seq.fetch_add(1, Ordering::Relaxed);
            let frame = ServerFrame::event($event, $payload, s);
            let _ = sender
                .send(WsMessage::Text(
                    serde_json::to_string(&frame).unwrap().into(),
                ))
                .await;
        }};
    }

    // --- Serialize concurrent executions ---
    let _run_guard = state.run_queue.acquire(&store_key).await;

    // --- Acquire session write lock ---
    if let Err(lock_err) = state.write_lock.try_acquire(&store_key, conn_id).await {
        let err = ServerFrame::err(
            request_id_rpc,
            RpcError::invalid_request(format!("Session busy: {lock_err}")),
        );
        let _ = sender
            .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
            .await;
        return;
    }

    send_event!(
        "agent.message.start",
        serde_json::json!({
            "request_id": request_id,
            "sessionKey": session_key_str,
        })
    );

    // Session creation/resolution is handled by AgentSession.resolve_session()
    // inside handle_message_streaming_with_context(). No need to pre-create here.

    // --- Build InboundMessage ---
    let mut msg = InboundMessage::web(
        request_id.clone(),
        store_key.clone(),
        content.clone(),
        conn_id,
    );

    // Convert ws attachments to inbound message attachments
    if !ws_attachments.is_empty() {
        let mut env_attachments = Vec::new();
        let mut parts = vec![content.clone()];
        for att in &ws_attachments {
            env_attachments.push(EnvelopeAttachment {
                filename: att.filename.clone(),
                url: att.url.clone(),
                mime_type: Some(att.mime_type.clone()),
            });
            parts.push(format!(
                "\n[Attached: {} ({})]({}) ",
                att.filename, att.mime_type, att.url
            ));
        }
        msg.attachments = env_attachments;
        // Also embed attachment references in content for backwards compatibility
        msg.content = parts.join("");
    }

    msg.finalize();

    // --- Create WsStreamingOutput + RunContext ---
    let (frame_tx, mut frame_rx) = mpsc::unbounded_channel::<String>();
    let ws_output = Arc::new(WsStreamingOutput::new(
        frame_tx,
        Arc::new(AtomicU64::new(seq.load(Ordering::Relaxed))),
        request_id.clone(),
        session_key_str.to_string(),
    ));
    let streaming_handle = StreamingOutputHandle::new(ws_output);

    // Cancel token for this execution
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .write()
        .await
        .insert(store_key.clone(), cancel_tx);

    let ctx = RunContext {
        cancel_token: Some(cancel_rx.clone()),
        streaming_output: Some(Arc::new(streaming_handle)),
    };

    let execution_start = std::time::Instant::now();

    // --- Spawn agent execution task ---
    let agent_session = state.agent_session.clone();
    let mut agent_handle =
        tokio::spawn(async move { agent_session.handle_message(msg, ctx).await });

    // Drop span guard before async select loop
    drop(_req_guard);

    let mut final_content_text = String::new();

    // --- Main forwarding loop ---
    // Concurrently: forward WsStreamingOutput frames to WS sender,
    // handle incoming WS messages (cancel, approval, ping), and wait
    // for the agent task to finish.
    loop {
        tokio::select! {
            // Forward serialized frames from WsStreamingOutput to the WebSocket
            Some(frame_json) = frame_rx.recv() => {
                // Track content from delta events for the final response
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&frame_json) {
                    if let Some(event_name) = parsed.get("event").and_then(|v| v.as_str()) {
                        if event_name == "agent.message.delta" {
                            if let Some(payload) = parsed.get("payload") {
                                if let Some(chunk) = payload.get("content").and_then(|v| v.as_str()) {
                                    final_content_text.push_str(chunk);
                                }
                            }
                        }
                    }
                    // Update sequence counter
                    if let Some(s) = parsed.get("seq").and_then(|v| v.as_u64()) {
                        let _ = seq.fetch_max(s + 1, Ordering::Relaxed);
                    }
                }
                let _ = sender
                    .send(WsMessage::Text(frame_json.into()))
                    .await;
            }
            // Handle incoming WS messages during execution
            ws_result = receiver.next() => {
                match ws_result {
                    Some(Ok(WsMessage::Text(ref text))) => {
                        if let Ok(ClientFrame::Request { id, method, params: _ }) =
                            serde_json::from_str::<ClientFrame>(text)
                        {
                            match method.as_str() {
                                "chat.stop" => {
                                    if let Some(tx) = state.cancel_tokens.read().await.get(&store_key) {
                                        let _ = tx.send(true);
                                    }
                                    let ok = ServerFrame::ok(&id, serde_json::json!({"stopped": true}));
                                    let _ = sender
                                        .send(WsMessage::Text(
                                            serde_json::to_string(&ok).unwrap().into(),
                                        ))
                                        .await;
                                }
                                "ping" => {
                                    let ok = ServerFrame::ok(&id, serde_json::json!({"pong": true}));
                                    let _ = sender
                                        .send(WsMessage::Text(
                                            serde_json::to_string(&ok).unwrap().into(),
                                        ))
                                        .await;
                                }
                                _ => {
                                    // Ignore other methods during agent execution
                                }
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => {
                        // Client disconnected
                        break;
                    }
                    Some(Err(_)) => {
                        // WebSocket error — disconnect
                        break;
                    }
                    _ => {} // skip binary/ping/pong
                }
            }
            // Agent task completed
            result = &mut agent_handle => {
                // Drain remaining frames from the channel
                frame_rx.close();
                while let Some(frame_json) = frame_rx.recv().await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&frame_json) {
                        if let Some(event_name) = parsed.get("event").and_then(|v| v.as_str()) {
                            if event_name == "agent.message.delta" {
                                if let Some(payload) = parsed.get("payload") {
                                    if let Some(chunk) = payload.get("content").and_then(|v| v.as_str()) {
                                        final_content_text.push_str(chunk);
                                    }
                                }
                            }
                        }
                    }
                    let _ = sender
                        .send(WsMessage::Text(frame_json.into()))
                        .await;
                }

                // Session token count is updated inside AgentSession::handle_message()
                // using the per-request token values returned by the agent.

                let elapsed = execution_start.elapsed().as_millis();

                match result {
                    Ok(Ok(_reply)) => {
                        let _g = req_span.enter();
                        tracing::info!(duration_ms = %elapsed, "v3 turn completed");
                    }
                    Ok(Err(e)) => {
                        let _g = req_span.enter();
                        tracing::error!(error = %e, duration_ms = %elapsed, "v3 agent execution failed");
                    }
                    Err(e) => {
                        let _g = req_span.enter();
                        tracing::error!(error = %e, "v3 agent task panicked");
                    }
                }

                // Emit MessageSent (fire-and-forget)
                {
                    let event_bus = state.event_bus.clone();
                    let req_id = request_id.clone();
                    let sk = session_key_str.to_string();
                    tokio::spawn(async move {
                        let mut event = Event::new(
                            EventKind::MessageSent,
                            serde_json::json!({
                                "request_id": req_id,
                                "session_key": sk,
                                "channel": "web",
                                "protocol": "v3",
                            }),
                        )
                        .with_source("gateway/ws");
                        let _ = event_bus.emit(&mut event).await;
                    });
                }

                break;
            }
        }
    }

    // --- Cleanup ---
    state.write_lock.release(&store_key, conn_id).await;

    // Send the final RPC response for the agent/chat.send request
    let response = ServerFrame::ok(
        request_id_rpc,
        serde_json::json!({
            "request_id": request_id,
            "content": final_content_text,
            "sessionKey": session_key_str,
        }),
    );
    let _ = sender
        .send(WsMessage::Text(
            serde_json::to_string(&response).unwrap().into(),
        ))
        .await;
}
