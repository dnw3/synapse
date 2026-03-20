use serde::{Deserialize, Serialize};

/// Usage data included in the `done` event.
#[derive(Serialize)]
pub(crate) struct DoneUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

/// WebSocket event types sent from server to client.
#[derive(Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub(crate) enum WsEvent {
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
        session_key: String,
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
pub(crate) struct Attachment {
    #[allow(dead_code)]
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub url: String,
}

/// WebSocket commands from client to server.
#[derive(Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub(crate) enum WsCommand {
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
    /// Start a voice input session.
    #[serde(rename = "voice_start")]
    VoiceStart { format: String },
    /// Append a base64-encoded audio chunk to the active voice session.
    #[serde(rename = "voice_chunk")]
    VoiceChunk { data: String },
    /// End the current voice input session.
    #[serde(rename = "voice_end")]
    VoiceEnd,
}
