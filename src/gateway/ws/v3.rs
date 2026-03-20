use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use synaptic::core::{ChatModel, MemoryStore, Message};
use synaptic::events::{Event, EventKind};
use synaptic::graph::{MessageState, StreamMode};
use tokio::sync::mpsc;
use tracing::Instrument;
use uuid::Uuid;

use super::streaming::{extract_canvas_directives, StreamingProxy};
use super::types::Attachment;
use super::utils::{find_tool_name, load_session_overrides, truncate, ws_json};
use crate::agent::build_deep_agent_with_callback;
use crate::agent::callbacks::{ApprovalResponse, WebSocketApprovalCallback};
use crate::gateway::rpc::{
    AuthResult, ClientFrame, ConnectParams, FeatureInfo, HelloOk, Role, RpcContext, RpcError,
    ServerFrame, ServerInfo, SnapshotInfo, StateVersion, GATEWAY_EVENTS, PROTOCOL_VERSION,
};
use crate::gateway::state::AppState;

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, conversation_id, state))
}

async fn handle_socket(socket: WebSocket, conversation_id: String, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Unique identifier for this WebSocket connection (used as lock holder).
    let conn_id = Uuid::new_v4().to_string();

    tracing::info!(%conn_id, %conversation_id, "websocket connected");

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
        handle_v3_connection(sender, receiver, conversation_id, state, conn_id, first_msg).await;
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
    conversation_id: String,
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

    tracing::info!(%conn_id, %conversation_id, ?role, "v3 connection established");

    // --- Create cancel channel ---
    let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .write()
        .await
        .insert(conversation_id.clone(), cancel_tx);

    // Event sequence counter for this connection
    let seq = AtomicU64::new(1);

    // Tick timer for keepalive / heartbeat
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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
                            // Handle agent execution — reuse existing streaming logic
                            handle_v3_agent(
                                &mut sender,
                                &mut receiver,
                                &conversation_id,
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
    state.cancel_tokens.write().await.remove(&conversation_id);
    state.write_lock.release(&conversation_id, &conn_id).await;
    tracing::info!(%conn_id, %conversation_id, "v3 connection closed");
}

/// Handle an `agent` or `chat.send` RPC request in v3 protocol.
///
/// Reuses the existing `StreamingProxy` + agent builder logic. Streams
/// tokens/reasoning/tool events as v3 `ServerFrame::Event`, then sends
/// the final result as a `ServerFrame::Response`.
#[allow(clippy::too_many_arguments)]
async fn handle_v3_agent(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    receiver: &mut SplitStream<WebSocket>,
    conversation_id: &str,
    state: &AppState,
    conn_id: &str,
    request_id_rpc: &str,
    params: &Value,
    seq: &AtomicU64,
) {
    let request_id = synaptic::logging::generate_request_id();

    let req_span = tracing::info_span!(
        "ws_v3_request",
        %request_id,
        %conn_id,
        %conversation_id,
    );
    let _req_guard = req_span.enter();

    // Extract message content from params
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let attachments: Vec<Attachment> = params
        .get("attachments")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if content.is_empty() && attachments.is_empty() {
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
        "v3 agent request"
    );

    // Emit MessageReceived event (fire-and-forget)
    {
        let event_bus = state.event_bus.clone();
        let req_id = request_id.clone();
        let conv_id = conversation_id.to_string();
        tokio::spawn(async move {
            let mut event = Event::new(
                EventKind::MessageReceived,
                serde_json::json!({
                    "request_id": req_id,
                    "conversation_id": conv_id,
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

    // Serialize concurrent executions for the same session
    let _run_guard = state.run_queue.acquire(conversation_id).await;

    // Acquire session write lock
    if let Err(lock_err) = state.write_lock.try_acquire(conversation_id, conn_id).await {
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
        serde_json::json!({"request_id": request_id})
    );

    let memory = state.sessions.memory();

    // Ensure session exists
    if state
        .sessions
        .get_session(conversation_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        match state.sessions.create_session().await {
            Ok(session_id) => {
                // Tag the new session as the main web session.
                if let Ok(Some(mut info)) = state.sessions.get_session(&session_id).await {
                    info.session_key = Some("agent:default:main".to_string());
                    info.channel = Some("web".to_string());
                    info.chat_type = Some("direct".to_string());
                    info.display_name = Some("main".to_string());
                    let _ = state.sessions.update_session(&info).await;
                }
                // Emit SessionStart (fire-and-forget)
                let event_bus = state.event_bus.clone();
                let conv_id = conversation_id.to_string();
                tokio::spawn(async move {
                    let mut event = Event::new(
                        EventKind::SessionStart,
                        serde_json::json!({
                            "session_id": session_id,
                            "conversation_id": conv_id,
                            "channel": "web",
                            "protocol": "v3",
                        }),
                    )
                    .with_source("gateway/ws");
                    let _ = event_bus.emit(&mut event).await;
                });
            }
            Err(e) => {
                let err = ServerFrame::err(request_id_rpc, RpcError::internal(e.to_string()));
                let _ = sender
                    .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
                    .await;
                state.write_lock.release(conversation_id, conn_id).await;
                return;
            }
        }
    }

    let (token_tx, mut token_rx) = mpsc::unbounded_channel::<String>();
    let (reasoning_tx, mut reasoning_rx) = mpsc::unbounded_channel::<String>();
    let proxy_model: Arc<dyn ChatModel> = Arc::new(StreamingProxy {
        inner: state.model.clone(),
        token_tx,
        reasoning_tx,
    });

    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let checkpointer = Arc::new(state.sessions.checkpointer());
    let overrides = load_session_overrides(conversation_id);
    let show_reasoning = overrides
        .as_ref()
        .and_then(|o| o.thinking.as_deref())
        .unwrap_or("off")
        != "off";
    let (approval_cb, mut approval_rx, approval_resp_tx) = WebSocketApprovalCallback::new();
    let agent = match build_deep_agent_with_callback(
        proxy_model,
        &state.config,
        &cwd,
        checkpointer,
        state.mcp_tools.clone(),
        None,
        Some(approval_cb),
        None,
        None,
        overrides,
        Some(state.cost_tracker.clone()),
        "web",
        None,
        Some(state.event_bus.clone()),
        Some(state.plugin_registry.clone()),
        Some(state.channel_registry.clone()),
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, "v3 agent build failed");
            let err = ServerFrame::err(request_id_rpc, RpcError::internal(e.to_string()));
            let _ = sender
                .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
                .await;
            state.write_lock.release(conversation_id, conn_id).await;
            return;
        }
    };

    tracing::info!("v3 agent execution started");

    let approval_resp_tx = Arc::new(tokio::sync::Mutex::new(Some(approval_resp_tx)));

    // Build final content with attachment references
    let final_content = if attachments.is_empty() {
        content.clone()
    } else {
        let mut parts = vec![content.clone()];
        for att in &attachments {
            parts.push(format!(
                "\n[Attached: {} ({})]({}) ",
                att.filename, att.mime_type, att.url
            ));
        }
        parts.join("")
    };

    let mut messages = memory.load(conversation_id).await.unwrap_or_default();
    if !messages.iter().any(|m| m.is_system()) {
        if let Some(ref prompt) = state.config.base.agent.system_prompt {
            messages.insert(0, Message::system(prompt));
        }
    }
    messages.push(
        Message::human(&final_content)
            .with_additional_kwarg("request_id", serde_json::Value::String(request_id.clone())),
    );

    let initial_state = MessageState::with_messages(messages);
    let pre_snap = state.cost_tracker.snapshot().await;
    let pre_tokens = pre_snap.total_input_tokens + pre_snap.total_output_tokens;
    let mut stream = agent.stream(initial_state, StreamMode::Values);

    let mut displayed = 0usize;
    let mut token_buffer = String::new();
    let mut token_flush_interval: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
    let execution_start = std::time::Instant::now();

    // Create a cancel channel for this execution
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .write()
        .await
        .insert(conversation_id.to_string(), cancel_tx);

    // Drop span guard before async select loop
    drop(_req_guard);

    let mut final_content_text = String::new();

    loop {
        tokio::select! {
            Some(token) = token_rx.recv() => {
                token_buffer.push_str(&token);
                if token_flush_interval.is_none() {
                    token_flush_interval = Some(Box::pin(tokio::time::sleep(
                        std::time::Duration::from_millis(150),
                    )));
                }
            }
            Some(reasoning) = reasoning_rx.recv() => {
                if show_reasoning {
                    send_event!("agent.thinking.delta", serde_json::json!({
                        "content": reasoning
                    }));
                }
            }
            _ = async { token_flush_interval.as_mut().unwrap().await }, if token_flush_interval.is_some() => {
                if !token_buffer.is_empty() {
                    let chunk = std::mem::take(&mut token_buffer);
                    final_content_text.push_str(&chunk);
                    send_event!("agent.message.delta", serde_json::json!({
                        "type": "text",
                        "content": chunk
                    }));
                }
                token_flush_interval = None;
            }
            Some(req) = approval_rx.recv() => {
                send_event!("approval.requested", serde_json::json!({
                    "tool_name": req.tool_name,
                    "args_preview": req.args_preview,
                    "risk_level": req.risk_level,
                }));
            }
            Some(Ok(ws_msg)) = receiver.next() => {
                if let WsMessage::Text(ref text) = ws_msg {
                    // Try v3 frame first, then fall back to legacy commands
                    if let Ok(ClientFrame::Request { id, method, params }) =
                        serde_json::from_str::<ClientFrame>(text)
                    {
                        match method.as_str() {
                            "chat.stop" => {
                                if let Some(tx) = state.cancel_tokens.read().await.get(conversation_id) {
                                    let _ = tx.send(true);
                                }
                                let ok = ServerFrame::ok(&id, serde_json::json!({"stopped": true}));
                                let _ = sender
                                    .send(WsMessage::Text(
                                        serde_json::to_string(&ok).unwrap().into(),
                                    ))
                                    .await;
                            }
                            "approval.approve" | "approval.deny" => {
                                let approved = method == "approval.approve";
                                let allow_all = params.get("allow_all")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                if let Some(tx) = approval_resp_tx.lock().await.as_ref() {
                                    let _ = tx.send(ApprovalResponse { approved, allow_all });
                                }
                                let ok = ServerFrame::ok(&id, serde_json::json!({"ok": true}));
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
            }
            event = stream.next().instrument(req_span.clone()) => {
                match event {
                    Some(Ok(graph_event)) => {
                        let msgs = &graph_event.state.messages;
                        for msg in msgs.iter().skip(displayed) {
                            if msg.is_ai() {
                                let tool_calls = msg.tool_calls();
                                if !tool_calls.is_empty() {
                                    for tc in tool_calls {
                                        tracing::debug!(tool = %tc.name, "tool call");
                                        send_event!("agent.tool.start", serde_json::json!({
                                            "name": tc.name,
                                            "args": tc.arguments,
                                        }));
                                    }
                                } else {
                                    let content = msg.content();
                                    for canvas_evt in extract_canvas_directives(content, &state.canvas_engine) {
                                        let _ = sender.send(ws_json(&canvas_evt)).await;
                                    }
                                }
                            } else if msg.is_tool() {
                                let tool_name = find_tool_name(msgs, displayed, msg);
                                tracing::debug!(tool = %tool_name, "tool result");
                                send_event!("agent.tool.result", serde_json::json!({
                                    "name": tool_name,
                                    "content": truncate(msg.content(), 500),
                                }));
                            }
                            displayed += 1;
                        }
                        let saved = memory.load(conversation_id).await.map(|m| m.len()).unwrap_or(0);
                        let new_msgs: Vec<_> = msgs.iter().skip(saved).collect();
                        let last_ai_idx = new_msgs.iter().rposition(|m| m.is_ai());
                        for (i, msg) in new_msgs.iter().enumerate() {
                            let msg = if last_ai_idx == Some(i) {
                                (*msg).clone().with_additional_kwarg(
                                    "request_id",
                                    serde_json::Value::String(request_id.clone()),
                                )
                            } else {
                                (*msg).clone()
                            };
                            memory.append(conversation_id, msg).await.ok();
                        }
                    }
                    Some(Err(e)) => {
                        let _g = req_span.enter();
                        tracing::error!(error = %e, "v3 agent execution failed");
                        if !token_buffer.is_empty() {
                            let chunk = std::mem::take(&mut token_buffer);
                            final_content_text.push_str(&chunk);
                            send_event!("agent.message.delta", serde_json::json!({
                                "type": "text",
                                "content": chunk
                            }));
                        }
                        send_event!("agent.error", serde_json::json!({
                            "message": e.to_string(),
                            "request_id": request_id,
                        }));
                        break;
                    }
                    None => {
                        // Stream complete — drain remaining tokens
                        token_rx.close();
                        reasoning_rx.close();
                        while let Some(token) = token_rx.recv().await {
                            token_buffer.push_str(&token);
                        }
                        if !token_buffer.is_empty() {
                            let chunk = std::mem::take(&mut token_buffer);
                            final_content_text.push_str(&chunk);
                            send_event!("agent.message.delta", serde_json::json!({
                                "type": "text",
                                "content": chunk
                            }));
                        }
                        while let Some(r) = reasoning_rx.recv().await {
                            if show_reasoning {
                                send_event!("agent.thinking.delta", serde_json::json!({
                                    "content": r
                                }));
                            }
                        }
                        // Update session token count
                        {
                            let snap = state.cost_tracker.snapshot().await;
                            let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                            let delta = post_tokens.saturating_sub(pre_tokens);
                            if delta > 0 {
                                if let Ok(Some(mut info)) = state.sessions.get_session(conversation_id).await {
                                    info.total_tokens += delta;
                                    let _ = state.sessions.update_session(&info).await;
                                }
                            }
                        }
                        let elapsed = execution_start.elapsed().as_millis();
                        let _g = req_span.enter();
                        tracing::info!(duration_ms = %elapsed, "v3 turn completed");
                        drop(_g);

                        // Usage tracking is handled by CostTrackingSubscriber via EventBus.

                        send_event!("agent.turn.complete", serde_json::json!({
                            "request_id": request_id,
                        }));

                        // Emit MessageSent (fire-and-forget)
                        {
                            let event_bus = state.event_bus.clone();
                            let req_id = request_id.clone();
                            let conv_id = conversation_id.to_string();
                            tokio::spawn(async move {
                                let mut event = Event::new(
                                    EventKind::MessageSent,
                                    serde_json::json!({
                                        "request_id": req_id,
                                        "conversation_id": conv_id,
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
            _ = cancel_rx.changed() => {
                let _g = req_span.enter();
                if *cancel_rx.borrow() {
                    if !token_buffer.is_empty() {
                        let chunk = std::mem::take(&mut token_buffer);
                        final_content_text.push_str(&chunk);
                        send_event!("agent.message.delta", serde_json::json!({
                            "type": "text",
                            "content": chunk
                        }));
                    }
                    let elapsed = execution_start.elapsed().as_millis();
                    tracing::info!(duration_ms = %elapsed, "v3 execution cancelled");
                    send_event!("agent.message.complete", serde_json::json!({
                        "request_id": request_id,
                        "cancelled": true,
                    }));
                    break;
                }
            }
        }
    }

    // Cleanup after agent execution
    drop(approval_resp_tx);
    state.write_lock.release(conversation_id, conn_id).await;

    // Send the final RPC response for the agent/chat.send request
    let response = ServerFrame::ok(
        request_id_rpc,
        serde_json::json!({
            "request_id": request_id,
            "content": final_content_text,
            "conversation_id": conversation_id,
        }),
    );
    let _ = sender
        .send(WsMessage::Text(
            serde_json::to_string(&response).unwrap().into(),
        ))
        .await;
}
