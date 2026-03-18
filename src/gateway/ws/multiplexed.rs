use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use synaptic::core::{ChatModel, MemoryStore, Message};
use synaptic::events::{Event, EventKind};
use synaptic::graph::{MessageState, StreamMode};
use tokio::sync::mpsc;
use tracing::Instrument;
use uuid::Uuid;

use super::streaming::StreamingProxy;
use super::utils::{find_tool_name, load_session_overrides, truncate};
use crate::agent::build_deep_agent_with_callback;
use crate::gateway::rpc::{ClientFrame, ClientInfo, Role, RpcContext, RpcError, ServerFrame};
use crate::gateway::state::AppState;

pub(crate) async fn ws_multiplexed_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_multiplexed_socket(socket, state))
}

async fn handle_multiplexed_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let conn_id = Uuid::new_v4().to_string();

    tracing::info!(%conn_id, "multiplexed websocket connected");

    // Register with broadcaster so we receive server-push events
    let mut event_rx = state.broadcaster.register(conn_id.clone()).await;

    // Build an RpcContext with anonymous/default client info.
    // The multiplexed endpoint skips the connect handshake — clients that
    // need auth should use the per-conversation route or pass a token via
    // query params in a future iteration.
    let rpc_ctx = Arc::new(RpcContext {
        state: state.clone(),
        conn_id: conn_id.clone(),
        client: ClientInfo::default(),
        role: Role::Operator,
        scopes: std::collections::HashSet::new(),
        broadcaster: state.broadcaster.clone(),
    });

    // Event sequence counter for this connection
    let seq = std::sync::atomic::AtomicU64::new(1);

    // Send connect.challenge so the client knows the connection is live
    let nonce = Uuid::new_v4().to_string();
    let challenge_seq = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let challenge = ServerFrame::event(
        "connect.challenge",
        serde_json::json!({
            "nonce": nonce,
            "conn_id": conn_id,
            "ts": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            "methods": state.rpc_router.method_names(),
        }),
        challenge_seq,
    );
    if ws_sender
        .send(WsMessage::Text(
            serde_json::to_string(&challenge).unwrap().into(),
        ))
        .await
        .is_err()
    {
        tracing::warn!(%conn_id, "failed to send challenge, closing");
        state.broadcaster.unregister(&conn_id).await;
        return;
    }

    // Tick timer for keepalive
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Channel for outbound messages from spawned tasks (e.g. chat.send agent)
    let (out_tx, mut out_rx) = mpsc::channel::<String>(256);

    loop {
        tokio::select! {
            // Periodic tick keepalive
            _ = tick_interval.tick() => {
                let tick_seq = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let tick_frame = ServerFrame::event("tick", serde_json::json!({
                    "ts": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                }), tick_seq);
                if ws_sender
                    .send(WsMessage::Text(serde_json::to_string(&tick_frame).unwrap().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            // Outbound messages from spawned agent tasks
            Some(msg) = out_rx.recv() => {
                if ws_sender.send(WsMessage::Text(msg.into())).await.is_err() {
                    break;
                }
            }

            // Events from Broadcaster (server-push to all connections)
            Some(frame) = event_rx.recv() => {
                if ws_sender
                    .send(WsMessage::Text(serde_json::to_string(&frame).unwrap().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            // Client frames from WebSocket
            msg = ws_receiver.next() => {
                let text = match msg {
                    Some(Ok(WsMessage::Text(t))) => t.to_string(),
                    Some(Ok(WsMessage::Close(_))) | None => {
                        tracing::info!(%conn_id, "multiplexed client disconnected");
                        break;
                    }
                    Some(Ok(WsMessage::Ping(_))) => {
                        // Respond with pong (axum handles this automatically, but be safe)
                        continue;
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        tracing::warn!(%conn_id, error = %e, "multiplexed websocket error");
                        break;
                    }
                };

                // Parse as ClientFrame (reuse existing v3 frame format)
                let frame = match serde_json::from_str::<ClientFrame>(&text) {
                    Ok(f) => f,
                    Err(e) => {
                        let err_frame = ServerFrame::err(
                            "unknown",
                            RpcError::invalid_request(format!("Invalid frame: {e}")),
                        );
                        if ws_sender
                            .send(WsMessage::Text(serde_json::to_string(&err_frame).unwrap().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                };

                match frame {
                    ClientFrame::Request { id, method, params } => {
                        if method == "chat.send" {
                            // Handle chat.send — extract session_key, echo back for now.
                            // Full agent integration will be wired in a follow-up.
                            let session_key = params
                                .get("session_key")
                                .or_else(|| params.get("sessionKey"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("default")
                                .to_string();
                            let content = params
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            // Ack immediately
                            let ack = ServerFrame::ok(
                                &id,
                                serde_json::json!({
                                    "status": "accepted",
                                    "session_key": session_key,
                                }),
                            );
                            if ws_sender
                                .send(WsMessage::Text(serde_json::to_string(&ack).unwrap().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            // Spawn async task to run the agent and stream events back
                            let out = out_tx.clone();
                            let sk = session_key.clone();
                            let st = state.clone();
                            tokio::spawn(async move {
                                run_agent_for_session(st, sk, content, out).await;
                            });
                        } else if method == "ping" {
                            let pong = ServerFrame::ok(&id, serde_json::json!({"pong": true}));
                            if ws_sender
                                .send(WsMessage::Text(serde_json::to_string(&pong).unwrap().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        } else {
                            // Standard RPC dispatch via rpc_router
                            let response = state
                                .rpc_router
                                .dispatch(rpc_ctx.clone(), id, &method, params)
                                .await;
                            if ws_sender
                                .send(WsMessage::Text(serde_json::to_string(&response).unwrap().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Cleanup
    state.broadcaster.unregister(&conn_id).await;
    tracing::info!(%conn_id, "multiplexed websocket closed");
}

/// Run the deep agent for a given session key, streaming events back via
/// `out_tx`. Used by the multiplexed WS handler's `chat.send` branch.
///
/// Events sent over `out_tx` use the multiplexed event format:
///   `{"type":"event","event":"agent.turn.chunk","payload":{...}}`
///
/// This is a simplified version of `handle_v3_agent` that does not support
/// mid-execution approval/cancel interactions (those require a bidirectional
/// channel, which can be added as a follow-up). The session_key is used as
/// the conversation_id so messages are persisted per-session.
#[allow(clippy::too_many_lines)]
async fn run_agent_for_session(
    state: AppState,
    session_key: String,
    content: String,
    out_tx: mpsc::Sender<String>,
) {
    let conversation_id = session_key.clone();
    let request_id = synaptic::logging::generate_request_id();

    let req_span = tracing::info_span!(
        "ws_mux_request",
        %request_id,
        session_key = %conversation_id,
    );
    let _req_guard = req_span.enter();

    tracing::info!(content_len = content.len(), "multiplexed chat.send request");

    // Helper macro to send an event frame via out_tx
    macro_rules! send_mux_event {
        ($event:expr, $payload:expr) => {{
            let frame = serde_json::json!({
                "type": "event",
                "event": $event,
                "payload": $payload,
                "seq": 0_u64,
            });
            let _ = out_tx.send(frame.to_string()).await;
        }};
    }

    // Emit thinking status
    send_mux_event!(
        "agent.turn.status",
        serde_json::json!({ "session_key": conversation_id, "state": "thinking" })
    );

    // Emit MessageReceived event (fire-and-forget)
    {
        let event_bus = state.event_bus.clone();
        let req_id = request_id.clone();
        let conv_id = conversation_id.clone();
        tokio::spawn(async move {
            let mut event = Event::new(
                EventKind::MessageReceived,
                serde_json::json!({
                    "request_id": req_id,
                    "conversation_id": conv_id,
                    "channel": "web",
                    "protocol": "multiplexed",
                }),
            )
            .with_source("gateway/ws/mux");
            let _ = event_bus.emit(&mut event).await;
        });
    }

    let memory = state.sessions.memory();

    // Ensure session exists
    if state
        .sessions
        .get_session(&conversation_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        match state.sessions.create_session().await {
            Ok(session_id) => {
                if let Ok(Some(mut info)) = state.sessions.get_session(&session_id).await {
                    info.session_key = Some(conversation_id.clone());
                    info.channel = Some("web".to_string());
                    info.chat_type = Some("direct".to_string());
                    info.display_name = Some(session_key.clone());
                    let _ = state.sessions.update_session(&info).await;
                }
                // Emit SessionStart (fire-and-forget)
                let event_bus = state.event_bus.clone();
                let conv_id = conversation_id.clone();
                tokio::spawn(async move {
                    let mut event = Event::new(
                        EventKind::SessionStart,
                        serde_json::json!({
                            "session_id": session_id,
                            "conversation_id": conv_id,
                            "channel": "web",
                            "protocol": "multiplexed",
                        }),
                    )
                    .with_source("gateway/ws/mux");
                    let _ = event_bus.emit(&mut event).await;
                });
            }
            Err(e) => {
                send_mux_event!(
                    "agent.turn.error",
                    serde_json::json!({
                        "session_key": conversation_id,
                        "message": e.to_string(),
                        "request_id": request_id,
                    })
                );
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
    let overrides = load_session_overrides(&conversation_id);
    let show_reasoning = overrides
        .as_ref()
        .and_then(|o| o.thinking.as_deref())
        .unwrap_or("off")
        != "off";

    // Build agent (no approval callback for multiplexed sessions; can be added later)
    let agent = match build_deep_agent_with_callback(
        proxy_model,
        &state.config,
        &cwd,
        checkpointer,
        state.mcp_tools.clone(),
        None,
        None, // no approval callback
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
            tracing::error!(error = %e, "multiplexed agent build failed");
            send_mux_event!(
                "agent.turn.error",
                serde_json::json!({
                    "session_key": conversation_id,
                    "message": e.to_string(),
                    "request_id": request_id,
                })
            );
            return;
        }
    };

    tracing::info!("multiplexed agent execution started");

    let mut messages = memory.load(&conversation_id).await.unwrap_or_default();
    if !messages.iter().any(|m| m.is_system()) {
        if let Some(ref prompt) = state.config.base.agent.system_prompt {
            messages.insert(0, Message::system(prompt));
        }
    }
    messages.push(
        Message::human(&content)
            .with_additional_kwarg("request_id", serde_json::Value::String(request_id.clone())),
    );

    let initial_state = MessageState::with_messages(messages);
    let pre_snap = state.cost_tracker.snapshot().await;
    let pre_tokens = pre_snap.total_input_tokens + pre_snap.total_output_tokens;
    let mut stream = agent.stream(initial_state, StreamMode::Values);

    send_mux_event!(
        "agent.turn.status",
        serde_json::json!({ "session_key": conversation_id, "state": "executing" })
    );

    let mut displayed = 0usize;
    let mut token_buffer = String::new();
    let mut token_flush_interval: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
    let mut final_content_text = String::new();
    let execution_start = std::time::Instant::now();

    // Register cancel channel
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .write()
        .await
        .insert(conversation_id.clone(), cancel_tx);

    drop(_req_guard);

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
                    send_mux_event!(
                        "agent.thinking.delta",
                        serde_json::json!({ "session_key": conversation_id, "content": reasoning })
                    );
                }
            }
            _ = async { token_flush_interval.as_mut().unwrap().await }, if token_flush_interval.is_some() => {
                if !token_buffer.is_empty() {
                    let chunk = std::mem::take(&mut token_buffer);
                    final_content_text.push_str(&chunk);
                    send_mux_event!(
                        "agent.turn.chunk",
                        serde_json::json!({ "session_key": conversation_id, "content": chunk })
                    );
                }
                token_flush_interval = None;
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
                                        tracing::debug!(tool = %tc.name, "multiplexed tool call");
                                        send_mux_event!(
                                            "agent.tool.start",
                                            serde_json::json!({
                                                "session_key": conversation_id,
                                                "name": tc.name,
                                                "args": tc.arguments,
                                            })
                                        );
                                    }
                                }
                            } else if msg.is_tool() {
                                let tool_name = find_tool_name(msgs, displayed, msg);
                                tracing::debug!(tool = %tool_name, "multiplexed tool result");
                                send_mux_event!(
                                    "agent.tool.result",
                                    serde_json::json!({
                                        "session_key": conversation_id,
                                        "name": tool_name,
                                        "content": truncate(msg.content(), 500),
                                    })
                                );
                            }
                            displayed += 1;
                        }
                        let saved = memory.load(&conversation_id).await.map(|m| m.len()).unwrap_or(0);
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
                            memory.append(&conversation_id, msg).await.ok();
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!(error = %e, "multiplexed agent execution failed");
                        if !token_buffer.is_empty() {
                            let chunk = std::mem::take(&mut token_buffer);
                            final_content_text.push_str(&chunk);
                            send_mux_event!(
                                "agent.turn.chunk",
                                serde_json::json!({ "session_key": conversation_id, "content": chunk })
                            );
                        }
                        send_mux_event!(
                            "agent.turn.error",
                            serde_json::json!({
                                "session_key": conversation_id,
                                "message": e.to_string(),
                                "request_id": request_id,
                            })
                        );
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
                            send_mux_event!(
                                "agent.turn.chunk",
                                serde_json::json!({ "session_key": conversation_id, "content": chunk })
                            );
                        }
                        while let Some(r) = reasoning_rx.recv().await {
                            if show_reasoning {
                                send_mux_event!(
                                    "agent.thinking.delta",
                                    serde_json::json!({ "session_key": conversation_id, "content": r })
                                );
                            }
                        }
                        // Update session token count
                        {
                            let snap = state.cost_tracker.snapshot().await;
                            let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                            let delta = post_tokens.saturating_sub(pre_tokens);
                            if delta > 0 {
                                if let Ok(Some(mut info)) = state.sessions.get_session(&conversation_id).await {
                                    info.total_tokens += delta;
                                    let _ = state.sessions.update_session(&info).await;
                                }
                            }
                        }
                        let elapsed = execution_start.elapsed().as_millis();
                        tracing::info!(duration_ms = %elapsed, "multiplexed turn completed");

                        send_mux_event!(
                            "agent.turn.complete",
                            serde_json::json!({
                                "session_key": conversation_id,
                                "request_id": request_id,
                            })
                        );

                        // Emit MessageSent (fire-and-forget)
                        {
                            let event_bus = state.event_bus.clone();
                            let req_id = request_id.clone();
                            let conv_id = conversation_id.clone();
                            tokio::spawn(async move {
                                let mut event = Event::new(
                                    EventKind::MessageSent,
                                    serde_json::json!({
                                        "request_id": req_id,
                                        "conversation_id": conv_id,
                                        "channel": "web",
                                        "protocol": "multiplexed",
                                    }),
                                )
                                .with_source("gateway/ws/mux");
                                let _ = event_bus.emit(&mut event).await;
                            });
                        }

                        break;
                    }
                }
            }
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    if !token_buffer.is_empty() {
                        let chunk = std::mem::take(&mut token_buffer);
                        final_content_text.push_str(&chunk);
                        send_mux_event!(
                            "agent.turn.chunk",
                            serde_json::json!({ "session_key": conversation_id, "content": chunk })
                        );
                    }
                    let elapsed = execution_start.elapsed().as_millis();
                    tracing::info!(duration_ms = %elapsed, "multiplexed execution cancelled");
                    send_mux_event!(
                        "agent.turn.complete",
                        serde_json::json!({
                            "session_key": conversation_id,
                            "request_id": request_id,
                            "cancelled": true,
                        })
                    );
                    break;
                }
            }
        }
    }

    // Remove cancel token
    state.cancel_tokens.write().await.remove(&conversation_id);
}
