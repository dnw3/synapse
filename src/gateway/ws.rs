use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, MemoryStore, Message, ModelProfile,
    SynapticError, ToolCall,
};
use synaptic::graph::{MessageState, StreamMode};
use tokio::sync::mpsc;
use tracing::Instrument;
use uuid::Uuid;

use regex::Regex;

use crate::agent::callbacks::{ApprovalRequest, ApprovalResponse, WebSocketApprovalCallback};
use crate::agent::{build_deep_agent_with_callback, SessionOverrides};
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
    Done {},
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

    let mut processed_idempotency_keys: HashSet<String> = HashSet::new();

    tracing::info!(%conn_id, %conversation_id, "websocket connected");

    // Send hello event with server capabilities
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

    while let Some(Ok(msg)) = receiver.next().await {
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

                // Per-request span: all logs in this request inherit request_id, conn_id, conversation_id
                let req_span = tracing::info_span!(
                    "ws_request",
                    %request_id,
                    %conn_id,
                    %conversation_id,
                );
                let _req_guard = req_span.enter();

                // Deduplicate by idempotency key (OpenClaw pattern)
                if let Some(ref key) = idempotency_key {
                    if !processed_idempotency_keys.insert(key.clone()) {
                        tracing::warn!(idempotency_key = %key, "duplicate message deduplicated");
                        let _ = sender.send(ws_json(&WsEvent::Done {})).await;
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
                                    {
                                        let snap = state.cost_tracker.snapshot().await;
                                        let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                                        let delta = post_tokens.saturating_sub(pre_tokens);
                                        if delta > 0 {
                                            if let Ok(Some(mut info)) = state.sessions.get_session(&conversation_id).await {
                                                info.token_count += delta;
                                                let _ = state.sessions.update_session(&info).await;
                                            }
                                        }
                                    }
                                    let elapsed = execution_start.elapsed().as_millis();
                                    tracing::info!(duration_ms = %elapsed, "turn completed");
                                    let _ = sender.send(ws_json(&WsEvent::Done {})).await;
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
                                    {
                                        let snap = state.cost_tracker.snapshot().await;
                                        let post_tokens = snap.total_input_tokens + snap.total_output_tokens;
                                        let delta = post_tokens.saturating_sub(pre_tokens);
                                        if delta > 0 {
                                            if let Ok(Some(mut info)) = state.sessions.get_session(&conversation_id).await {
                                                info.token_count += delta;
                                                let _ = state.sessions.update_session(&info).await;
                                            }
                                        }
                                    }
                                    let elapsed = execution_start.elapsed().as_millis();
                                    tracing::info!(duration_ms = %elapsed, "turn completed");
                                    let _ = sender.send(ws_json(&WsEvent::Done {})).await;
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
