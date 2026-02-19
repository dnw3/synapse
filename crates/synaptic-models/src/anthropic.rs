use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapticError,
    TokenUsage, ToolCall, ToolChoice, ToolDefinition,
};

use crate::backend::{ProviderBackend, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl AnthropicConfig {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://api.anthropic.com".to_string(),
            max_tokens: 1024,
            top_p: None,
            stop: None,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_top_p(mut self, top_p: f64) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }
}

pub struct AnthropicChatModel {
    config: AnthropicConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl AnthropicChatModel {
    pub fn new(config: AnthropicConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    fn build_request(&self, request: &ChatRequest, stream: bool) -> ProviderRequest {
        let mut system_text: Option<String> = None;
        let mut messages: Vec<Value> = Vec::new();

        for msg in &request.messages {
            match msg {
                Message::System { content, .. } => {
                    system_text = Some(content.clone());
                }
                Message::Human { content, .. } => {
                    messages.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut content_blocks: Vec<Value> = Vec::new();
                    if !content.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": content,
                        }));
                    }
                    for tc in tool_calls {
                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments,
                        }));
                    }
                    messages.push(json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                }
                Message::Tool {
                    content,
                    tool_call_id,
                    ..
                } => {
                    messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content,
                        }],
                    }));
                }
                Message::Chat {
                    custom_role,
                    content,
                    ..
                } => {
                    messages.push(json!({
                        "role": custom_role,
                        "content": content,
                    }));
                }
                Message::Remove { .. } => { /* skip Remove messages */ }
            }
        }

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": messages,
            "stream": stream,
        });

        if let Some(system) = system_text {
            body["system"] = json!(system);
        }

        if let Some(top_p) = self.config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(ref stop) = self.config.stop {
            body["stop_sequences"] = json!(stop);
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(request
                .tools
                .iter()
                .map(tool_def_to_anthropic)
                .collect::<Vec<_>>());
        }
        if let Some(ref choice) = request.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!({"type": "auto"}),
                ToolChoice::Required => json!({"type": "any"}),
                ToolChoice::None => json!({"type": "none"}),
                ToolChoice::Specific(name) => json!({"type": "tool", "name": name}),
            };
        }

        ProviderRequest {
            url: format!("{}/v1/messages", self.config.base_url),
            headers: vec![
                ("x-api-key".to_string(), self.config.api_key.clone()),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body,
        }
    }
}

fn tool_def_to_anthropic(def: &ToolDefinition) -> Value {
    json!({
        "name": def.name,
        "description": def.description,
        "input_schema": def.parameters,
    })
}

fn parse_response(resp: &ProviderResponse) -> Result<ChatResponse, SynapticError> {
    check_error_status(resp)?;

    let content_blocks = resp.body["content"].as_array().cloned().unwrap_or_default();

    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for block in &content_blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(t) = block["text"].as_str() {
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                if let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) {
                    tool_calls.push(ToolCall {
                        id: id.to_string(),
                        name: name.to_string(),
                        arguments: block["input"].clone(),
                    });
                }
            }
            _ => {}
        }
    }

    let usage = parse_usage(&resp.body["usage"]);

    let message = if tool_calls.is_empty() {
        Message::ai(text)
    } else {
        Message::ai_with_tool_calls(text, tool_calls)
    };

    Ok(ChatResponse { message, usage })
}

fn check_error_status(resp: &ProviderResponse) -> Result<(), SynapticError> {
    if resp.status == 429 {
        let msg = resp.body["error"]["message"]
            .as_str()
            .unwrap_or("rate limited")
            .to_string();
        return Err(SynapticError::RateLimit(msg));
    }
    if resp.status >= 400 {
        let msg = resp.body["error"]["message"]
            .as_str()
            .unwrap_or("unknown API error")
            .to_string();
        return Err(SynapticError::Model(format!(
            "Anthropic API error ({}): {}",
            resp.status, msg
        )));
    }
    Ok(())
}

fn parse_usage(usage: &Value) -> Option<TokenUsage> {
    if usage.is_null() {
        return None;
    }
    Some(TokenUsage {
        input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
        output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
        total_tokens: (usage["input_tokens"].as_u64().unwrap_or(0)
            + usage["output_tokens"].as_u64().unwrap_or(0)) as u32,
        input_details: None,
        output_details: None,
    })
}

fn parse_stream_event(event_type: &str, data: &str) -> Option<AIMessageChunk> {
    let v: Value = serde_json::from_str(data).ok()?;

    match event_type {
        "content_block_delta" => {
            let delta = &v["delta"];
            match delta["type"].as_str() {
                Some("text_delta") => Some(AIMessageChunk {
                    content: delta["text"].as_str().unwrap_or("").to_string(),
                    ..Default::default()
                }),
                Some("input_json_delta") => {
                    // Tool input streaming — we accumulate partial JSON
                    // For simplicity, we emit tool_calls once in content_block_start
                    None
                }
                _ => None,
            }
        }
        "content_block_start" => {
            let block = &v["content_block"];
            if block["type"].as_str() == Some("tool_use") {
                let id = block["id"].as_str().unwrap_or("").to_string();
                let name = block["name"].as_str().unwrap_or("").to_string();
                Some(AIMessageChunk {
                    tool_calls: vec![ToolCall {
                        id,
                        name,
                        arguments: block["input"].clone(),
                    }],
                    ..Default::default()
                })
            } else {
                None
            }
        }
        "message_delta" => {
            let usage = parse_usage(&v["usage"]);
            if usage.is_some() {
                Some(AIMessageChunk {
                    usage,
                    ..Default::default()
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

#[async_trait]
impl ChatModel for AnthropicChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let provider_req = self.build_request(&request, false);
        let resp = self.backend.send(provider_req).await?;
        parse_response(&resp)
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        Box::pin(async_stream::stream! {
            let provider_req = self.build_request(&request, true);
            let byte_stream = self.backend.send_stream(provider_req).await;

            let byte_stream = match byte_stream {
                Ok(s) => s,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            use eventsource_stream::Eventsource;
            use futures::StreamExt;

            let mut event_stream = byte_stream
                .map(|result| result.map_err(|e| std::io::Error::other(e.to_string())))
                .eventsource();

            while let Some(event) = event_stream.next().await {
                match event {
                    Ok(ev) => {
                        if ev.event == "message_stop" {
                            break;
                        }
                        if let Some(chunk) = parse_stream_event(&ev.event, &ev.data) {
                            yield Ok(chunk);
                        }
                    }
                    Err(e) => {
                        yield Err(SynapticError::Model(format!("SSE parse error: {e}")));
                        break;
                    }
                }
            }
        })
    }
}
