//! WebSocket streaming output implementation.
//!
//! Bridges the [`StreamingOutput`] trait to a live WebSocket connection by
//! sending protocol v3 [`ServerFrame`] events through an unbounded mpsc channel.
//!
//! Token output is throttled: tokens are accumulated in a buffer and flushed
//! at most once every 150 ms to avoid flooding the client.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use synaptic::graph::streaming::{CompletionMeta, StreamingOutput, ToolCallInfo};
use tokio::sync::{mpsc, Mutex};

use crate::gateway::rpc::ServerFrame;

/// Maximum interval between token flushes.
const TOKEN_FLUSH_INTERVAL: Duration = Duration::from_millis(150);

// ---------------------------------------------------------------------------
// TokenBuffer — accumulates text and tracks flush timing
// ---------------------------------------------------------------------------

struct TokenBuffer {
    buf: String,
    last_flush: Instant,
}

impl TokenBuffer {
    fn new() -> Self {
        Self {
            buf: String::new(),
            last_flush: Instant::now(),
        }
    }

    /// Push new text into the buffer. Returns `Some(chunk)` if the 150 ms
    /// throttle window has elapsed and the buffer should be flushed now.
    fn push(&mut self, token: &str) -> Option<String> {
        self.buf.push_str(token);
        if self.last_flush.elapsed() >= TOKEN_FLUSH_INTERVAL && !self.buf.is_empty() {
            Some(self.take())
        } else {
            None
        }
    }

    /// Drain the buffer regardless of elapsed time. Returns `None` if empty.
    fn flush(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            None
        } else {
            Some(self.take())
        }
    }

    fn take(&mut self) -> String {
        self.last_flush = Instant::now();
        std::mem::take(&mut self.buf)
    }
}

// ---------------------------------------------------------------------------
// WsStreamingOutput
// ---------------------------------------------------------------------------

/// Implements [`StreamingOutput`] by forwarding protocol v3 events to a
/// WebSocket sender channel.
///
/// Methods are synchronous from the caller's perspective (fire-and-forget):
/// serialisation happens inline but the send never blocks.
pub struct WsStreamingOutput {
    /// Serialised JSON frames are queued here for the WS sender task.
    tx: mpsc::UnboundedSender<String>,
    /// Per-connection monotonically increasing sequence counter.
    seq: Arc<AtomicU64>,
    /// Request id included in completion / error events.
    request_id: String,
    /// Token buffer with 150 ms flush throttle.
    buffer: Mutex<TokenBuffer>,
}

impl WsStreamingOutput {
    /// Create a new output handle.
    ///
    /// * `tx` — unbounded sender for serialised JSON `ServerFrame` strings.
    /// * `seq` — shared sequence counter for the connection.
    /// * `request_id` — included in relevant event payloads.
    pub fn new(
        tx: mpsc::UnboundedSender<String>,
        seq: Arc<AtomicU64>,
        request_id: impl Into<String>,
    ) -> Self {
        Self {
            tx,
            seq,
            request_id: request_id.into(),
            buffer: Mutex::new(TokenBuffer::new()),
        }
    }

    /// Serialize a `ServerFrame::Event` and enqueue it for delivery.
    fn send_event(&self, event: &str, payload: serde_json::Value) {
        let s = self.seq.fetch_add(1, Ordering::Relaxed);
        let frame = ServerFrame::event(event, payload, s);
        if let Ok(json) = serde_json::to_string(&frame) {
            let _ = self.tx.send(json);
        }
    }

    /// Flush any buffered tokens as an `agent.message.delta` event.
    async fn flush_buffer(&self) {
        let mut buf = self.buffer.lock().await;
        if let Some(chunk) = buf.flush() {
            drop(buf);
            self.send_event(
                "agent.message.delta",
                serde_json::json!({ "type": "text", "content": chunk }),
            );
        }
    }
}

#[async_trait]
impl StreamingOutput for WsStreamingOutput {
    /// Accumulate token into the 150 ms throttle buffer.
    ///
    /// If the throttle window has elapsed the entire buffer is flushed
    /// immediately; otherwise the token waits until the next flush trigger
    /// (`on_complete` / `on_error`) or the next token that crosses the
    /// window boundary.
    async fn on_token(&self, token: &str) {
        let mut buf = self.buffer.lock().await;
        if let Some(chunk) = buf.push(token) {
            drop(buf);
            self.send_event(
                "agent.message.delta",
                serde_json::json!({ "type": "text", "content": chunk }),
            );
        }
    }

    /// Reasoning / thinking delta — sent immediately as `agent.thinking.delta`.
    async fn on_reasoning(&self, content: &str) {
        self.send_event(
            "agent.thinking.delta",
            serde_json::json!({ "content": content }),
        );
    }

    /// Tool invocation start — sent immediately as `agent.tool.start`.
    async fn on_tool_call(&self, info: &ToolCallInfo) {
        self.send_event(
            "agent.tool.start",
            serde_json::json!({
                "name": info.name,
                "id":   info.id,
                "args": info.args,
            }),
        );
    }

    /// Tool result — sent immediately as `agent.tool.result`.
    async fn on_tool_result(&self, name: &str, content: &str) {
        self.send_event(
            "agent.tool.result",
            serde_json::json!({ "name": name, "content": content }),
        );
    }

    /// Flush token buffer, then send `agent.turn.complete`.
    async fn on_complete(&self, _full_response: &str, meta: Option<&CompletionMeta>) {
        self.flush_buffer().await;

        let mut payload = serde_json::json!({ "request_id": self.request_id });
        if let Some(m) = meta {
            payload["input_tokens"] = m.input_tokens.into();
            payload["output_tokens"] = m.output_tokens.into();
            payload["duration_ms"] = m.duration_ms.into();
        }
        self.send_event("agent.turn.complete", payload);
    }

    /// Flush token buffer, then send `agent.error`.
    async fn on_error(&self, error: &str) {
        self.flush_buffer().await;
        self.send_event(
            "agent.error",
            serde_json::json!({
                "message":    error,
                "request_id": self.request_id,
            }),
        );
    }

    /// Heartbeat ping — sent immediately as `agent.heartbeat`.
    async fn on_heartbeat(&self) {
        self.send_event("agent.heartbeat", serde_json::json!({}));
    }
}
