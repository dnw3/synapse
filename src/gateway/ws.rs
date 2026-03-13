use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, MemoryStore, Message, ModelProfile,
    SynapticError, ToolCall,
};
use synaptic::graph::{MessageState, StreamMode};
use tokio::sync::mpsc;
use tracing::Instrument;
use uuid::Uuid;

use regex::Regex;

use crate::agent::callbacks::{ApprovalResponse, WebSocketApprovalCallback};
use crate::agent::{build_deep_agent_with_callback, SessionOverrides};
use crate::gateway::messages::{Attachment as EnvelopeAttachment, MessageEnvelope};
use crate::gateway::rpc::{
    AuthResult, ClientFrame, ConnectParams, FeatureInfo, HelloOk, Role, RpcContext, RpcError,
    ServerFrame, ServerInfo, SnapshotInfo, StateVersion, GATEWAY_EVENTS, PROTOCOL_VERSION,
};
use crate::gateway::state::AppState;

// ---------------------------------------------------------------------------
// Streaming proxy: wraps a ChatModel, uses stream_chat() internally,
// and forwards token deltas to a channel for real-time WS delivery.
// ---------------------------------------------------------------------------

struct StreamingProxy {
    inner: Arc<dyn ChatModel>,
    token_tx: mpsc::UnboundedSender<String>,
    reasoning_tx: mpsc::UnboundedSender<String>,
}

#[async_trait]
impl ChatModel for StreamingProxy {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let mut stream = self.inner.stream_chat(request);
        let mut content = String::new();
        // Accumulate tool call chunks by index — streaming sends partial data:
        // chunk 1: id + name + partial args, chunk 2+: only partial args
        let mut tc_map: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new(); // index -> (id, name, args_buffer)
        let mut usage = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if !chunk.content.is_empty() {
                let _ = self.token_tx.send(chunk.content.clone());
                content.push_str(&chunk.content);
            }
            if !chunk.reasoning.is_empty() {
                let _ = self.reasoning_tx.send(chunk.reasoning.clone());
            }
            // Merge tool call chunks by index
            for tc in &chunk.tool_call_chunks {
                let idx = tc.index.unwrap_or(0);
                let entry = tc_map
                    .entry(idx)
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
                if let Some(ref id) = tc.id {
                    entry.0.clone_from(id);
                }
                if let Some(ref name) = tc.name {
                    entry.1.clone_from(name);
                }
                if let Some(ref args) = tc.arguments {
                    entry.2.push_str(args);
                }
            }
            if chunk.usage.is_some() {
                usage = chunk.usage;
            }
        }

        // Build final tool calls from accumulated chunks
        let tool_calls: Vec<ToolCall> = tc_map
            .into_values()
            .filter(|(_, name, _)| !name.is_empty())
            .map(|(id, name, args_buf)| {
                let arguments = if args_buf.is_empty() {
                    serde_json::Value::Object(Default::default())
                } else {
                    serde_json::from_str(&args_buf)
                        .unwrap_or(serde_json::Value::Object(Default::default()))
                };
                ToolCall {
                    id,
                    name,
                    arguments,
                }
            })
            .collect();

        Ok(ChatResponse {
            message: Message::ai_with_tool_calls(content, tool_calls),
            usage,
        })
    }

    fn profile(&self) -> Option<ModelProfile> {
        self.inner.profile()
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        self.inner.stream_chat(request)
    }
}

/// Parse `[canvas:type attrs]content[/canvas]` directives from text.
fn extract_canvas_directives(text: &str) -> Vec<WsEvent> {
    let re = Regex::new(r"\[canvas:(\w+)([^\]]*)\]([\s\S]*?)\[/canvas\]").unwrap();
    let attr_re = Regex::new(r"(\w+)=(\S+)").unwrap();
    let mut events = Vec::new();

    for cap in re.captures_iter(text) {
        let block_type = cap[1].to_string();
        let attrs_str = cap[2].trim();
        let content = cap[3].to_string();

        let mut attrs = serde_json::Map::new();
        for am in attr_re.captures_iter(attrs_str) {
            attrs.insert(
                am[1].to_string(),
                serde_json::Value::String(am[2].to_string()),
            );
        }

        let language = attrs
            .remove("lang")
            .and_then(|v| v.as_str().map(String::from));
        let attributes = if attrs.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(attrs))
        };

        events.push(WsEvent::CanvasUpdate {
            block_type,
            content,
            language,
            attributes,
        });
    }

    events
}

/// Usage data included in the `done` event.
#[derive(Serialize)]
struct DoneUsage {
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: f64,
}

/// WebSocket event types sent from server to client.
#[derive(Serialize)]
#[serde(tag = "type")]
enum WsEvent {
    #[serde(rename = "token")]
    Token { content: String },
    #[serde(rename = "reasoning")]
    Reasoning { content: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult { name: String, content: String },
    #[serde(rename = "status")]
    Status {
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    #[serde(rename = "canvas_update")]
    CanvasUpdate {
        block_type: String,
        content: String,
        language: Option<String>,
        attributes: Option<serde_json::Value>,
    },
    #[serde(rename = "approval_request")]
    ApprovalRequest {
        tool_name: String,
        args_preview: String,
        risk_level: String,
    },
    #[serde(rename = "subagent_complete")]
    SubagentComplete { task_id: String, summary: String },
    #[serde(rename = "done")]
    Done {
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<DoneUsage>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    /// Hello event sent immediately on connection.
    #[serde(rename = "hello")]
    Hello {
        version: String,
        features: Vec<String>,
        conversation_id: String,
    },
    /// RPC response to a client request.
    #[serde(rename = "rpc_response")]
    RpcResponse {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Attachment sent with a message.
#[derive(Deserialize, Clone)]
struct Attachment {
    #[allow(dead_code)]
    id: String,
    filename: String,
    mime_type: String,
    url: String,
}

/// WebSocket commands from client to server.
#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsCommand {
    #[serde(rename = "message")]
    SendMessage {
        content: String,
        #[serde(default)]
        attachments: Vec<Attachment>,
        /// Optional idempotency key for deduplication (UUID from client).
        #[serde(default)]
        idempotency_key: Option<String>,
    },
    #[serde(rename = "form_submit")]
    FormSubmit {
        block_id: String,
        values: serde_json::Value,
    },
    #[serde(rename = "approval_response")]
    ApprovalResp {
        approved: bool,
        #[serde(default)]
        allow_all: bool,
    },
    #[serde(rename = "cancel")]
    Cancel {},
    /// RPC request from client.
    #[serde(rename = "rpc_request")]
    RpcRequest {
        id: String,
        method: String,
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Heartbeat ping from client.
    #[serde(rename = "ping")]
    Ping {},
}

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/ws/{conversation_id}", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
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
        Some(Ok(_)) => {
            // Binary or other non-text frame — treat as legacy
            handle_legacy_connection(sender, receiver, conversation_id, state, conn_id, None).await;
            return;
        }
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
        // Legacy protocol: pass the first message so it isn't lost
        handle_legacy_connection(
            sender,
            receiver,
            conversation_id,
            state,
            conn_id,
            Some(first_msg),
        )
        .await;
    }
}

// ---------------------------------------------------------------------------
// Legacy protocol handler (preserves all existing behaviour)
// ---------------------------------------------------------------------------

async fn handle_legacy_connection(
    mut sender: SplitSink<WebSocket, WsMessage>,
    mut receiver: SplitStream<WebSocket>,
    conversation_id: String,
    state: AppState,
    conn_id: String,
    first_msg: Option<String>,
) {
    let mut processed_idempotency_keys: HashSet<String> = HashSet::new();

    // Send legacy hello event with server capabilities
    let _ = sender
        .send(ws_json(&WsEvent::Hello {
            version: env!("CARGO_PKG_VERSION").to_string(),
            features: vec![
                "streaming".to_string(),
                "rpc".to_string(),
                "attachments".to_string(),
                "approval".to_string(),
                "canvas".to_string(),
                "subagents".to_string(),
            ],
            conversation_id: conversation_id.clone(),
        }))
        .await;

    // Notify client if there's an active execution for this conversation
    // (e.g. user switched away and back while the agent was still running).
    if state.write_lock.is_locked(&conversation_id).await {
        let _ = sender
            .send(ws_json(&WsEvent::Status {
                state: "executing".to_string(),
                request_id: None,
            }))
            .await;
    }

    // Create cancel channel
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .write()
        .await
        .insert(conversation_id.clone(), cancel_tx);

    let memory = state.sessions.memory();

    // Wrap the receiver so we can inject the first message that was already
    // consumed during protocol detection without borrowing receiver twice.
    let mut pending_first: Option<WsMessage> = first_msg.map(|text| WsMessage::Text(text.into()));

    while let Some(Ok(msg)) = {
        if let Some(m) = pending_first.take() {
            Some(Ok(m))
        } else {
            receiver.next().await
        }
    } {
        let WsMessage::Text(text) = msg else {
            continue;
        };

        let cmd: WsCommand = match serde_json::from_str(&text) {
            Ok(cmd) => cmd,
            Err(e) => {
                let event = WsEvent::Error {
                    message: format!("invalid command: {}", e),
                    request_id: None,
                };
                let _ = sender
                    .send(WsMessage::Text(
                        serde_json::to_string(&event).unwrap().into(),
                    ))
                    .await;
                continue;
            }
        };

        match cmd {
            WsCommand::Ping {} => {
                // Respond with pong (empty status event to confirm liveness)
                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "pong".to_string(),
                        request_id: None,
                    }))
                    .await;
                continue;
            }
            WsCommand::RpcRequest { id, method, params } => {
                let result = handle_rpc(&state, &conversation_id, &method, &params).await;
                let event = match result {
                    Ok(val) => WsEvent::RpcResponse {
                        id,
                        result: Some(val),
                        error: None,
                    },
                    Err(e) => WsEvent::RpcResponse {
                        id,
                        result: None,
                        error: Some(e),
                    },
                };
                let _ = sender.send(ws_json(&event)).await;
                continue;
            }
            WsCommand::SendMessage {
                content,
                attachments,
                idempotency_key,
            } => {
                let request_id = crate::logging::generate_request_id();

                // Construct a MessageEnvelope for standardized tracing/metadata
                let mut envelope = MessageEnvelope::webchat(
                    request_id.clone(),
                    conversation_id.clone(),
                    content.clone(),
                    &conn_id,
                );
                envelope.attachments = attachments
                    .iter()
                    .map(|a| EnvelopeAttachment {
                        filename: a.filename.clone(),
                        url: a.url.clone(),
                        mime_type: Some(a.mime_type.clone()),
                    })
                    .collect();
                envelope.idempotency_key = idempotency_key.clone();

                // Per-request span: all logs in this request inherit envelope metadata
                let req_span = tracing::info_span!(
                    "ws_request",
                    request_id = %envelope.request_id,
                    channel = %envelope.delivery.channel,
                    session_key = %envelope.session_key,
                    provenance = ?envelope.provenance.kind,
                    %conn_id,
                );
                let _req_guard = req_span.enter();

                // Deduplicate by idempotency key (OpenClaw pattern)
                if let Some(ref key) = idempotency_key {
                    if !processed_idempotency_keys.insert(key.clone()) {
                        tracing::warn!(idempotency_key = %key, "duplicate message deduplicated");
                        let _ = sender
                            .send(ws_json(&WsEvent::Done {
                                usage: None,
                                model: None,
                                stop_reason: None,
                            }))
                            .await;
                        continue;
                    }
                }

                // Truncate content for logging (avoid huge payloads in logs)
                let content_preview: String = content.chars().take(200).collect();
                let attachment_count = attachments.len();
                tracing::info!(
                    msg_type = "send_message",
                    content = %content_preview,
                    attachments = attachment_count,
                    "user message received"
                );
                let execution_start = std::time::Instant::now();

                // Acquire session write lock before processing.
                if let Err(lock_err) = state
                    .write_lock
                    .try_acquire(&conversation_id, &conn_id)
                    .await
                {
                    tracing::warn!("session busy, rejected");
                    let _ = sender
                        .send(ws_json(&WsEvent::Error {
                            message: format!("session is busy: {}", lock_err),
                            request_id: Some(request_id.clone()),
                        }))
                        .await;
                    continue;
                }

                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "thinking".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;

                if state
                    .sessions
                    .get_session(&conversation_id)
                    .await
                    .ok()
                    .flatten()
                    .is_none()
                {
                    if let Err(e) = state.sessions.create_session().await {
                        let _ = sender
                            .send(ws_json(&WsEvent::Error {
                                message: e.to_string(),
                                request_id: Some(request_id.clone()),
                            }))
                            .await;
                        continue;
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
                // Show reasoning when thinking is enabled (any level other than "off")
                let show_reasoning = overrides
                    .as_ref()
                    .and_then(|o| o.thinking.as_deref())
                    .unwrap_or("off")
                    != "off";
                let (approval_cb, mut approval_rx, approval_resp_tx) =
                    WebSocketApprovalCallback::new();
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
                    None, // web UI uses default agent workspace
                )
                .await
                {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::error!(error = %e, "agent build failed");
                        let _ = sender
                            .send(ws_json(&WsEvent::Error {
                                message: e.to_string(),
                                request_id: Some(request_id.clone()),
                            }))
                            .await;
                        continue;
                    }
                };

                tracing::info!("agent execution started");

                // Store approval_resp_tx for incoming WS messages
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

                let mut messages = memory.load(&conversation_id).await.unwrap_or_default();
                if !messages.iter().any(|m| m.is_system()) {
                    if let Some(ref prompt) = state.config.base.agent.system_prompt {
                        messages.insert(0, Message::system(prompt));
                    }
                }
                // Add human message to state (persisted by the graph save loop)
                messages.push(Message::human(&final_content).with_additional_kwarg(
                    "request_id",
                    serde_json::Value::String(request_id.clone()),
                ));

                let initial_state = MessageState::with_messages(messages);
                // Snapshot token counts before execution so we can compute per-turn delta
                let pre_snap = state.cost_tracker.snapshot().await;
                let pre_tokens = pre_snap.total_input_tokens + pre_snap.total_output_tokens;
                let mut stream = agent.stream(initial_state, StreamMode::Values);

                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "executing".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;

                let mut displayed = 0usize;
                let mut token_buffer = String::new();
                let mut token_flush_interval: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

                // Drop the span guard before the select loop to avoid holding it across awaits.
                // Re-enter via req_span.enter() in each select arm that logs.
                drop(_req_guard);

                loop {
                    tokio::select! {
                        // Forward streaming tokens from LLM proxy
                        Some(token) = token_rx.recv() => {
                            token_buffer.push_str(&token);
                            if token_flush_interval.is_none() {
                                token_flush_interval = Some(Box::pin(tokio::time::sleep(std::time::Duration::from_millis(150))));
                            }
                        }
                        // Forward reasoning/thinking deltas (gated by session override)
                        Some(reasoning) = reasoning_rx.recv() => {
                            if show_reasoning {
                                let _ = sender.send(ws_json(&WsEvent::Reasoning {
                                    content: reasoning,
                                })).await;
                            }
                        }
                        // Flush buffered tokens at 150ms intervals
                        _ = async { token_flush_interval.as_mut().unwrap().await }, if token_flush_interval.is_some() => {
                            if !token_buffer.is_empty() {
                                let _ = sender.send(ws_json(&WsEvent::Token {
                                    content: std::mem::take(&mut token_buffer),
                                })).await;
                            }
                            token_flush_interval = None;
                        }
                        // Forward approval requests to the WS client
                        Some(req) = approval_rx.recv() => {
                            let _ = sender.send(ws_json(&WsEvent::ApprovalRequest {
                                tool_name: req.tool_name,
                                args_preview: req.args_preview,
                                risk_level: req.risk_level,
                            })).await;
                        }
                        // Handle incoming WS messages during agent execution
                        // (approval responses, cancel, RPC, ping)
                        Some(Ok(ws_msg)) = receiver.next() => {
                            if let WsMessage::Text(ref text) = ws_msg {
                                if let Ok(cmd) = serde_json::from_str::<WsCommand>(text) {
                                    match cmd {
                                        WsCommand::ApprovalResp { approved, allow_all } => {
                                            if let Some(tx) = approval_resp_tx.lock().await.as_ref() {
                                                let _ = tx.send(ApprovalResponse { approved, allow_all });
                                            }
                                        }
                                        WsCommand::Cancel {} => {
                                            if let Some(tx) = state.cancel_tokens.read().await.get(&conversation_id) {
                                                let _ = tx.send(true);
                                            }
                                        }
                                        WsCommand::RpcRequest { id, method, params } => {
                                            let result = handle_rpc(&state, &conversation_id, &method, &params).await;
                                            let event = match result {
                                                Ok(val) => WsEvent::RpcResponse { id, result: Some(val), error: None },
                                                Err(e) => WsEvent::RpcResponse { id, result: None, error: Some(e) },
                                            };
                                            let _ = sender.send(ws_json(&event)).await;
                                        }
                                        WsCommand::Ping {} => {
                                            let _ = sender.send(ws_json(&WsEvent::Status {
                                                state: "pong".to_string(),
                                                request_id: None,
                                            })).await;
                                        }
                                        _ => {} // Ignore other commands during execution
                                    }
                                }
                            }
                        }
                        // Instrument the stream future so all internal async work
                        // (model calls, tool calls via middleware) inherits the request span.
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
                                                    let _ = sender.send(ws_json(&WsEvent::ToolCall {
                                                        name: tc.name.clone(),
                                                        args: tc.arguments.clone(),
                                                    })).await;
                                                }
                                            } else {
                                                let content = msg.content();
                                                for canvas_evt in extract_canvas_directives(content) {
                                                    let _ = sender.send(ws_json(&canvas_evt)).await;
                                                }
                                            }
                                        } else if msg.is_tool() {
                                            let tool_name = find_tool_name(msgs, displayed, msg);
                                            tracing::debug!(tool = %tool_name, "tool result");
                                            let _ = sender.send(ws_json(&WsEvent::ToolResult {
                                                name: tool_name,
                                                content: truncate(msg.content(), 500),
                                            })).await;
                                        }
                                        displayed += 1;
                                    }
                                    let saved = memory.load(&conversation_id).await.map(|m| m.len()).unwrap_or(0);
                                    let new_msgs: Vec<_> = msgs.iter().skip(saved).collect();
                                    // Find the last AI message index to inject request_id only once
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
                                    tracing::error!(error = %e, "agent execution failed");
                                    // Final flush of buffered tokens
                                    if !token_buffer.is_empty() {
                                        let _ = sender.send(ws_json(&WsEvent::Token {
                                            content: std::mem::take(&mut token_buffer),
                                        })).await;
                                    }
                                    let _ = sender.send(ws_json(&WsEvent::Error {
                                        message: e.to_string(),
                                        request_id: Some(request_id.clone()),
                                    })).await;
                                    break;
                                }
                                None => {
                                    token_rx.close();
                                    reasoning_rx.close();
                                    while let Some(token) = token_rx.recv().await {
                                        token_buffer.push_str(&token);
                                    }
                                    // Final flush of buffered tokens
                                    if !token_buffer.is_empty() {
                                        let _ = sender.send(ws_json(&WsEvent::Token {
                                            content: std::mem::take(&mut token_buffer),
                                        })).await;
                                    }
                                    // Drain remaining reasoning (gated)
                                    while let Some(r) = reasoning_rx.recv().await {
                                        if show_reasoning {
                                            let _ = sender.send(ws_json(&WsEvent::Reasoning { content: r })).await;
                                        }
                                    }
                                    // Update session token count: add only this turn's delta
                                    let done_usage = {
                                        let snap = state.cost_tracker.snapshot().await;
                                        let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                                        let delta = post_tokens.saturating_sub(pre_tokens);
                                        if delta > 0 {
                                            if let Ok(Some(mut info)) = state.sessions.get_session(&conversation_id).await {
                                                info.token_count += delta;
                                                let _ = state.sessions.update_session(&info).await;
                                            }
                                        }
                                        // Compute per-turn usage from snapshot delta
                                        let turn_cost = (snap.estimated_cost_usd - pre_snap.estimated_cost_usd).max(0.0);
                                        DoneUsage {
                                            input_tokens: snap.total_input_tokens.saturating_sub(pre_snap.total_input_tokens),
                                            output_tokens: snap.total_output_tokens.saturating_sub(pre_snap.total_output_tokens),
                                            cost_usd: turn_cost,
                                        }
                                    };
                                    let model_name = state.model.profile().map(|p| p.name);
                                    let elapsed = execution_start.elapsed().as_millis();
                                    tracing::info!(duration_ms = %elapsed, "turn completed");
                                    let _ = sender.send(ws_json(&WsEvent::Done {
                                        usage: Some(done_usage),
                                        model: model_name,
                                        stop_reason: Some("end_turn".to_string()),
                                    })).await;
                                    break;
                                }
                            }
                        }
                        _ = cancel_rx.changed() => {
                            let _g = req_span.enter();
                            if *cancel_rx.borrow() {
                                // Final flush of buffered tokens
                                if !token_buffer.is_empty() {
                                    let _ = sender.send(ws_json(&WsEvent::Token {
                                        content: std::mem::take(&mut token_buffer),
                                    })).await;
                                }
                                let elapsed = execution_start.elapsed().as_millis();
                                tracing::info!(duration_ms = %elapsed, "execution cancelled by user");
                                let _ = sender.send(ws_json(&WsEvent::Status {
                                    state: "cancelled".to_string(),
                                    request_id: Some(request_id.clone()),
                                })).await;
                                break;
                            }
                        }
                    }
                }

                // Clean up approval channel
                drop(approval_resp_tx);

                // Release session write lock.
                state.write_lock.release(&conversation_id, &conn_id).await;

                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "idle".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;
            }
            WsCommand::FormSubmit { block_id, values } => {
                let request_id = crate::logging::generate_request_id();

                let req_span = tracing::info_span!(
                    "ws_request",
                    %request_id,
                    %conn_id,
                    %conversation_id,
                );
                let _req_guard = req_span.enter();

                tracing::info!(msg_type = "form_submit", "ws message received");
                let execution_start = std::time::Instant::now();

                if let Err(lock_err) = state
                    .write_lock
                    .try_acquire(&conversation_id, &conn_id)
                    .await
                {
                    tracing::warn!("session busy, rejected");
                    let _ = sender
                        .send(ws_json(&WsEvent::Error {
                            message: format!("session is busy: {}", lock_err),
                            request_id: Some(request_id.clone()),
                        }))
                        .await;
                    continue;
                }

                // Convert form submission into a user message for the agent
                let form_content = format!(
                    "[Form submitted: {}]\n{}",
                    block_id,
                    serde_json::to_string_pretty(&values).unwrap_or_default()
                );
                let human_msg = Message::human(&form_content);
                memory.append(&conversation_id, human_msg).await.ok();

                // Trigger agent processing (same flow as SendMessage)
                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "thinking".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;

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
                // Show reasoning when thinking is enabled (any level other than "off")
                let show_reasoning = overrides
                    .as_ref()
                    .and_then(|o| o.thinking.as_deref())
                    .unwrap_or("off")
                    != "off";
                let (approval_cb, mut approval_rx, approval_resp_tx) =
                    WebSocketApprovalCallback::new();
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
                    None, // web UI uses default agent workspace
                )
                .await
                {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::error!(error = %e, "agent build failed");
                        let _ = sender
                            .send(ws_json(&WsEvent::Error {
                                message: e.to_string(),
                                request_id: Some(request_id.clone()),
                            }))
                            .await;
                        continue;
                    }
                };

                tracing::info!("agent execution started");

                let approval_resp_tx = Arc::new(tokio::sync::Mutex::new(Some(approval_resp_tx)));

                let mut messages = memory.load(&conversation_id).await.unwrap_or_default();
                if !messages.iter().any(|m| m.is_system()) {
                    if let Some(ref prompt) = state.config.base.agent.system_prompt {
                        messages.insert(0, Message::system(prompt));
                    }
                }

                let initial_state = MessageState::with_messages(messages);
                // Snapshot token counts before execution so we can compute per-turn delta
                let pre_snap = state.cost_tracker.snapshot().await;
                let pre_tokens = pre_snap.total_input_tokens + pre_snap.total_output_tokens;
                let mut stream = agent.stream(initial_state, StreamMode::Values);

                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "executing".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;

                let mut displayed = 0usize;
                let mut token_buffer = String::new();
                let mut token_flush_interval: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

                // Drop the span guard before the select loop to avoid holding it across awaits.
                // Re-enter via req_span.enter() in each select arm that logs.
                drop(_req_guard);

                loop {
                    tokio::select! {
                        Some(token) = token_rx.recv() => {
                            token_buffer.push_str(&token);
                            if token_flush_interval.is_none() {
                                token_flush_interval = Some(Box::pin(tokio::time::sleep(std::time::Duration::from_millis(150))));
                            }
                        }
                        // Forward reasoning/thinking deltas (gated by session override)
                        Some(reasoning) = reasoning_rx.recv() => {
                            if show_reasoning {
                                let _ = sender.send(ws_json(&WsEvent::Reasoning {
                                    content: reasoning,
                                })).await;
                            }
                        }
                        // Flush buffered tokens at 150ms intervals
                        _ = async { token_flush_interval.as_mut().unwrap().await }, if token_flush_interval.is_some() => {
                            if !token_buffer.is_empty() {
                                let _ = sender.send(ws_json(&WsEvent::Token {
                                    content: std::mem::take(&mut token_buffer),
                                })).await;
                            }
                            token_flush_interval = None;
                        }
                        Some(req) = approval_rx.recv() => {
                            let _ = sender.send(ws_json(&WsEvent::ApprovalRequest {
                                tool_name: req.tool_name,
                                args_preview: req.args_preview,
                                risk_level: req.risk_level,
                            })).await;
                        }
                        Some(Ok(ws_msg)) = receiver.next() => {
                            if let WsMessage::Text(ref text) = ws_msg {
                                if let Ok(cmd) = serde_json::from_str::<WsCommand>(text) {
                                    match cmd {
                                        WsCommand::ApprovalResp { approved, allow_all } => {
                                            if let Some(tx) = approval_resp_tx.lock().await.as_ref() {
                                                let _ = tx.send(ApprovalResponse { approved, allow_all });
                                            }
                                        }
                                        WsCommand::Cancel {} => {
                                            if let Some(tx) = state.cancel_tokens.read().await.get(&conversation_id) {
                                                let _ = tx.send(true);
                                            }
                                        }
                                        WsCommand::RpcRequest { id, method, params } => {
                                            let result = handle_rpc(&state, &conversation_id, &method, &params).await;
                                            let event = match result {
                                                Ok(val) => WsEvent::RpcResponse { id, result: Some(val), error: None },
                                                Err(e) => WsEvent::RpcResponse { id, result: None, error: Some(e) },
                                            };
                                            let _ = sender.send(ws_json(&event)).await;
                                        }
                                        WsCommand::Ping {} => {
                                            let _ = sender.send(ws_json(&WsEvent::Status {
                                                state: "pong".to_string(),
                                                request_id: None,
                                            })).await;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        // Instrument the stream future so all internal async work
                        // (model calls, tool calls via middleware) inherits the request span.
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
                                                    let _ = sender.send(ws_json(&WsEvent::ToolCall {
                                                        name: tc.name.clone(),
                                                        args: tc.arguments.clone(),
                                                    })).await;
                                                }
                                            } else {
                                                let content = msg.content();
                                                for canvas_evt in extract_canvas_directives(content) {
                                                    let _ = sender.send(ws_json(&canvas_evt)).await;
                                                }
                                            }
                                        } else if msg.is_tool() {
                                            let tool_name = find_tool_name(msgs, displayed, msg);
                                            tracing::debug!(tool = %tool_name, "tool result");
                                            let _ = sender.send(ws_json(&WsEvent::ToolResult {
                                                name: tool_name,
                                                content: truncate(msg.content(), 500),
                                            })).await;
                                        }
                                        displayed += 1;
                                    }
                                    let saved = memory.load(&conversation_id).await.map(|m| m.len()).unwrap_or(0);
                                    let new_msgs: Vec<_> = msgs.iter().skip(saved).collect();
                                    // Find the last AI message index to inject request_id only once
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
                                    tracing::error!(error = %e, "agent execution failed");
                                    // Final flush of buffered tokens
                                    if !token_buffer.is_empty() {
                                        let _ = sender.send(ws_json(&WsEvent::Token {
                                            content: std::mem::take(&mut token_buffer),
                                        })).await;
                                    }
                                    let _ = sender.send(ws_json(&WsEvent::Error {
                                        message: e.to_string(),
                                        request_id: Some(request_id.clone()),
                                    })).await;
                                    break;
                                }
                                None => {
                                    token_rx.close();
                                    reasoning_rx.close();
                                    while let Some(token) = token_rx.recv().await {
                                        token_buffer.push_str(&token);
                                    }
                                    // Final flush of buffered tokens
                                    if !token_buffer.is_empty() {
                                        let _ = sender.send(ws_json(&WsEvent::Token {
                                            content: std::mem::take(&mut token_buffer),
                                        })).await;
                                    }
                                    // Drain remaining reasoning (gated)
                                    while let Some(r) = reasoning_rx.recv().await {
                                        if show_reasoning {
                                            let _ = sender.send(ws_json(&WsEvent::Reasoning { content: r })).await;
                                        }
                                    }
                                    // Update session token count: add only this turn's delta
                                    let done_usage = {
                                        let snap = state.cost_tracker.snapshot().await;
                                        let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                                        let delta = post_tokens.saturating_sub(pre_tokens);
                                        if delta > 0 {
                                            if let Ok(Some(mut info)) = state.sessions.get_session(&conversation_id).await {
                                                info.token_count += delta;
                                                let _ = state.sessions.update_session(&info).await;
                                            }
                                        }
                                        // Compute per-turn usage from snapshot delta
                                        let turn_cost = (snap.estimated_cost_usd - pre_snap.estimated_cost_usd).max(0.0);
                                        DoneUsage {
                                            input_tokens: snap.total_input_tokens.saturating_sub(pre_snap.total_input_tokens),
                                            output_tokens: snap.total_output_tokens.saturating_sub(pre_snap.total_output_tokens),
                                            cost_usd: turn_cost,
                                        }
                                    };
                                    let model_name = state.model.profile().map(|p| p.name);
                                    let elapsed = execution_start.elapsed().as_millis();
                                    tracing::info!(duration_ms = %elapsed, "turn completed");
                                    let _ = sender.send(ws_json(&WsEvent::Done {
                                        usage: Some(done_usage),
                                        model: model_name,
                                        stop_reason: Some("end_turn".to_string()),
                                    })).await;
                                    break;
                                }
                            }
                        }
                        _ = cancel_rx.changed() => {
                            let _g = req_span.enter();
                            if *cancel_rx.borrow() {
                                // Final flush of buffered tokens
                                if !token_buffer.is_empty() {
                                    let _ = sender.send(ws_json(&WsEvent::Token {
                                        content: std::mem::take(&mut token_buffer),
                                    })).await;
                                }
                                let elapsed = execution_start.elapsed().as_millis();
                                tracing::info!(duration_ms = %elapsed, "execution cancelled by user");
                                let _ = sender.send(ws_json(&WsEvent::Status {
                                    state: "cancelled".to_string(),
                                    request_id: Some(request_id.clone()),
                                })).await;
                                break;
                            }
                        }
                    }
                }

                drop(approval_resp_tx);

                // Release session write lock.
                state.write_lock.release(&conversation_id, &conn_id).await;

                let _ = sender
                    .send(ws_json(&WsEvent::Status {
                        state: "idle".to_string(),
                        request_id: Some(request_id.clone()),
                    }))
                    .await;
            }
            WsCommand::ApprovalResp { .. } => {
                // Approval responses are handled inside the agent execution loop
            }
            WsCommand::Cancel {} => {
                if let Some(tx) = state.cancel_tokens.read().await.get(&conversation_id) {
                    let _ = tx.send(true);
                }
            }
        }
    }

    // Cleanup: release write lock and cancel token.
    tracing::info!(%conn_id, %conversation_id, "websocket disconnected");
    state.write_lock.release(&conversation_id, &conn_id).await;
    state.cancel_tokens.write().await.remove(&conversation_id);
}

// ---------------------------------------------------------------------------
// Protocol v3 handler
// ---------------------------------------------------------------------------

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
        snapshot: SnapshotInfo {
            presence: serde_json::json!({}),
            health: serde_json::json!({"status": "ok"}),
            state_version: StateVersion::default(),
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
    let request_id = crate::logging::generate_request_id();

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
        if let Err(e) = state.sessions.create_session().await {
            let err = ServerFrame::err(request_id_rpc, RpcError::internal(e.to_string()));
            let _ = sender
                .send(WsMessage::Text(serde_json::to_string(&err).unwrap().into()))
                .await;
            state.write_lock.release(conversation_id, conn_id).await;
            return;
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
                                    for canvas_evt in extract_canvas_directives(content) {
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
                                    info.token_count += delta;
                                    let _ = state.sessions.update_session(&info).await;
                                }
                            }
                        }
                        let elapsed = execution_start.elapsed().as_millis();
                        tracing::info!(duration_ms = %elapsed, "v3 turn completed");

                        send_event!("agent.turn.complete", serde_json::json!({
                            "request_id": request_id,
                        }));
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

fn ws_json(event: &WsEvent) -> WsMessage {
    WsMessage::Text(serde_json::to_string(event).unwrap().into())
}

fn find_tool_name(messages: &[Message], displayed: usize, tool_msg: &Message) -> String {
    let tool_call_id = tool_msg.tool_call_id().unwrap_or_default();
    if tool_call_id.is_empty() {
        return "tool".to_string();
    }
    for msg in messages[..displayed].iter().rev() {
        if msg.is_ai() {
            for tc in msg.tool_calls() {
                if tc.id == tool_call_id {
                    return tc.name.clone();
                }
            }
        }
    }
    "tool".to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

/// Handle an RPC request from the client.
async fn handle_rpc(
    state: &AppState,
    conversation_id: &str,
    method: &str,
    _params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method {
        "get_status" => {
            let uptime = state.started_at.elapsed().as_secs();
            let auth_enabled = state
                .auth
                .as_ref()
                .map(|a| a.config.enabled)
                .unwrap_or(false);
            Ok(serde_json::json!({
                "status": "ok",
                "uptime_secs": uptime,
                "auth_enabled": auth_enabled,
                "conversation_id": conversation_id,
            }))
        }
        "get_messages" => {
            let memory = state.sessions.memory();
            let messages = memory.load(conversation_id).await.unwrap_or_default();
            let msg_list: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": if m.is_human() { "human" } else if m.is_ai() { "assistant" } else if m.is_system() { "system" } else { "tool" },
                        "content": m.content(),
                    })
                })
                .collect();
            Ok(serde_json::json!({ "messages": msg_list }))
        }
        "get_session_info" => {
            let memory = state.sessions.memory();
            let messages = memory.load(conversation_id).await.unwrap_or_default();
            let overrides = load_session_overrides(conversation_id);
            Ok(serde_json::json!({
                "conversation_id": conversation_id,
                "message_count": messages.len(),
                "thinking": overrides.as_ref().and_then(|o| o.thinking.as_deref()),
            }))
        }
        "check_execution" => {
            let is_executing = state.write_lock.is_locked(conversation_id).await;
            Ok(serde_json::json!({ "executing": is_executing }))
        }
        _ => Err(format!("unknown method: {}", method)),
    }
}

/// Load session overrides (thinking/verbose) from the dashboard overrides file.
fn load_session_overrides(conversation_id: &str) -> Option<SessionOverrides> {
    let path = std::path::PathBuf::from("data/session_overrides.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&content).ok()?;
    let entry = map.get(conversation_id)?;
    let thinking = entry
        .get("thinking")
        .and_then(|v| v.as_str())
        .map(String::from);
    let verbose = entry
        .get("verbose")
        .and_then(|v| v.as_str())
        .map(String::from);
    if thinking.is_none() && verbose.is_none() {
        return None;
    }
    Some(SessionOverrides { thinking, verbose })
}
