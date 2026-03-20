use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, ModelProfile, SynapticError, ToolCall,
};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Streaming proxy: wraps a ChatModel, uses stream_chat() internally,
// and forwards token deltas to a channel for real-time WS delivery.
// ---------------------------------------------------------------------------

pub(crate) struct StreamingProxy {
    pub inner: Arc<dyn ChatModel>,
    pub token_tx: mpsc::UnboundedSender<String>,
    pub reasoning_tx: mpsc::UnboundedSender<String>,
}

#[async_trait]
impl ChatModel for StreamingProxy {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        use futures::StreamExt;
        use synaptic::core::Message;

        let tool_names_req: Vec<&str> = request.tools.iter().map(|t| t.name.as_str()).collect();
        tracing::info!(
            message_count = request.messages.len(),
            tool_count = request.tools.len(),
            tool_names = ?tool_names_req,
            "StreamingProxy: starting model stream"
        );
        let mut stream = self.inner.stream_chat(request);
        let mut content = String::new();
        // Accumulate tool call chunks by index — streaming sends partial data:
        // chunk 1: id + name + partial args, chunk 2+: only partial args
        let mut tc_map: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new(); // index -> (id, name, args_buffer)
        let mut usage = None;
        let mut chunk_idx = 0usize;

        tracing::debug!("StreamingProxy: entering stream loop");
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tracing::debug!(
                chunk_idx,
                content_len = chunk.content.len(),
                content_preview = %if chunk.content.len() > 100 { &chunk.content[..100] } else { &chunk.content },
                reasoning_len = chunk.reasoning.len(),
                tool_call_chunks = chunk.tool_call_chunks.len(),
                has_usage = chunk.usage.is_some(),
                "StreamingProxy: chunk received"
            );
            chunk_idx += 1;
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
        tracing::debug!(
            chunk_idx,
            "StreamingProxy: stream loop exited (stream returned None)"
        );

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

        let tc_names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
        tracing::info!(
            content_len = content.len(),
            content_preview = %if content.len() > 200 { &content[..200] } else { &content },
            tool_call_count = tool_calls.len(),
            tool_names = ?tc_names,
            has_usage = usage.is_some(),
            "StreamingProxy: model stream completed"
        );

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
