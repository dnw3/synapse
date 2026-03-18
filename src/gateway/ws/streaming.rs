use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, ModelProfile, SynapticError, ToolCall,
};
use tokio::sync::mpsc;

use super::types::WsEvent;

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
/// When `engine` recognises the block type the raw content is replaced with
/// the rendered HTML produced by the matching [`CanvasRenderer`].
pub(crate) fn extract_canvas_directives(
    text: &str,
    engine: &crate::gateway::canvas::CanvasEngine,
) -> Vec<WsEvent> {
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

        // Build the data payload passed to the renderer: merge inline attrs with
        // the raw content so renderers can access both.
        let mut render_data = serde_json::json!({ "content": content });
        if let Some(serde_json::Value::Object(ref extra)) = attributes {
            for (k, v) in extra {
                render_data
                    .as_object_mut()
                    .unwrap()
                    .insert(k.clone(), v.clone());
            }
        }

        // Try the CanvasEngine first; fall back to raw content on miss.
        let (final_content, interactive) =
            if let Some(rendered) = engine.render(&block_type, &render_data) {
                (rendered.html, rendered.interactive)
            } else {
                (content, false)
            };

        let mut final_attrs = if interactive {
            // Surface the interactive flag via attributes so the frontend can
            // activate any embedded form/action logic.
            let mut m = serde_json::Map::new();
            m.insert("interactive".to_string(), serde_json::Value::Bool(true));
            // Merge original attrs back.
            if let Some(serde_json::Value::Object(orig)) = &attributes {
                for (k, v) in orig {
                    m.insert(k.clone(), v.clone());
                }
            }
            Some(serde_json::Value::Object(m))
        } else {
            attributes
        };

        // Remove "interactive" key from attributes if it was already there
        // to avoid duplicating it at the top level (already captured above).
        if let Some(serde_json::Value::Object(ref mut m)) = final_attrs {
            if !interactive {
                m.remove("interactive");
            }
        }

        events.push(WsEvent::CanvasUpdate {
            block_type,
            content: final_content,
            language,
            attributes: final_attrs,
        });
    }

    events
}
