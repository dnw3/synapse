use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use synaptic::core::RunContext;
use synaptic::deep::StreamingOutputHandle;
use synaptic::events::{Event, EventKind};
use tokio::sync::mpsc;

use super::streaming_output::WsStreamingOutput;
use super::types::Attachment;
use crate::gateway::messages::{Attachment as EnvelopeAttachment, InboundMessage};
use crate::gateway::rpc::{ClientFrame, RpcError, ServerFrame};
use crate::gateway::state::AppState;
use crate::session::key as session_key;

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
pub(super) async fn handle_v3_agent(
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
        let event_bus = state.infra.event_bus.clone();
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
    let _run_guard = state.session.run_queue.acquire(&store_key).await;

    // --- Acquire session write lock ---
    if let Err(lock_err) = state
        .session
        .write_lock
        .try_acquire(&store_key, conn_id)
        .await
    {
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
        .session
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
    let agent_session = state.agent.agent_session.clone();
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
                                    if let Some(tx) = state.session.cancel_tokens.read().await.get(&store_key) {
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
                    let event_bus = state.infra.event_bus.clone();
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
    state.session.write_lock.release(&store_key, conn_id).await;

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
