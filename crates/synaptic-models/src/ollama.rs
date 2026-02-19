use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapticError,
    TokenUsage, ToolCall, ToolChoice, ToolDefinition,
};

use crate::backend::{ProviderBackend, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub model: String,
    pub base_url: String,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
    pub seed: Option<u64>,
}

impl OllamaConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: "http://localhost:11434".to_string(),
            top_p: None,
            stop: None,
            seed: None,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
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

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
}

pub struct OllamaChatModel {
    config: OllamaConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl OllamaChatModel {
    pub fn new(config: OllamaConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    fn build_request(&self, request: &ChatRequest, stream: bool) -> ProviderRequest {
        let messages: Vec<Value> = request.messages.iter().map(message_to_ollama).collect();

        let mut body = json!({
            "model": self.config.model,
            "messages": messages,
            "stream": stream,
        });

        if !request.tools.is_empty() {
            body["tools"] = json!(request
                .tools
                .iter()
                .map(tool_def_to_ollama)
                .collect::<Vec<_>>());
        }
        if let Some(ref choice) = request.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::None => json!("none"),
                ToolChoice::Specific(name) => json!({
                    "type": "function",
                    "function": {"name": name}
                }),
            };
        }

        {
            let mut options = json!({});
            let mut has_options = false;
            if let Some(top_p) = self.config.top_p {
                options["top_p"] = json!(top_p);
                has_options = true;
            }
            if let Some(ref stop) = self.config.stop {
                options["stop"] = json!(stop);
                has_options = true;
            }
            if let Some(seed) = self.config.seed {
                options["seed"] = json!(seed);
                has_options = true;
            }
            if has_options {
                body["options"] = options;
            }
        }

        ProviderRequest {
            url: format!("{}/api/chat", self.config.base_url),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body,
        }
    }
}

fn message_to_ollama(msg: &Message) -> Value {
    match msg {
        Message::System { content, .. } => json!({
            "role": "system",
            "content": content,
        }),
        Message::Human { content, .. } => json!({
            "role": "user",
            "content": content,
        }),
        Message::AI {
            content,
            tool_calls,
            ..
        } => {
            let mut obj = json!({
                "role": "assistant",
                "content": content,
            });
            if !tool_calls.is_empty() {
                obj["tool_calls"] = json!(tool_calls
                    .iter()
                    .map(|tc| json!({
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments,
                        }
                    }))
                    .collect::<Vec<_>>());
            }
            obj
        }
        Message::Tool {
            content,
            tool_call_id: _,
            ..
        } => json!({
            "role": "tool",
            "content": content,
        }),
        Message::Chat {
            custom_role,
            content,
            ..
        } => json!({
            "role": custom_role,
            "content": content,
        }),
        Message::Remove { .. } => json!(null), // Remove messages are skipped
    }
}

fn tool_def_to_ollama(def: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": def.name,
            "description": def.description,
            "parameters": def.parameters,
        }
    })
}

fn parse_response(resp: &ProviderResponse) -> Result<ChatResponse, SynapticError> {
    check_error_status(resp)?;

    let message_val = &resp.body["message"];
    let content = message_val["content"].as_str().unwrap_or("").to_string();
    let tool_calls = parse_tool_calls(message_val);

    let usage = parse_usage(&resp.body);

    let message = if tool_calls.is_empty() {
        Message::ai(content)
    } else {
        Message::ai_with_tool_calls(content, tool_calls)
    };

    Ok(ChatResponse { message, usage })
}

fn check_error_status(resp: &ProviderResponse) -> Result<(), SynapticError> {
    if resp.status >= 400 {
        let msg = resp.body["error"]
            .as_str()
            .unwrap_or("unknown Ollama error")
            .to_string();
        return Err(SynapticError::Model(format!(
            "Ollama API error ({}): {}",
            resp.status, msg
        )));
    }
    Ok(())
}

fn parse_tool_calls(message: &Value) -> Vec<ToolCall> {
    message["tool_calls"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(i, tc)| {
                    let name = tc["function"]["name"].as_str()?.to_string();
                    let arguments = tc["function"]["arguments"].clone();
                    Some(ToolCall {
                        id: format!("ollama-{i}"),
                        name,
                        arguments,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_usage(body: &Value) -> Option<TokenUsage> {
    let prompt = body["prompt_eval_count"].as_u64();
    let completion = body["eval_count"].as_u64();
    match (prompt, completion) {
        (Some(p), Some(c)) => Some(TokenUsage {
            input_tokens: p as u32,
            output_tokens: c as u32,
            total_tokens: (p + c) as u32,
            input_details: None,
            output_details: None,
        }),
        _ => None,
    }
}

fn parse_ndjson_chunk(line: &str) -> Option<AIMessageChunk> {
    let v: Value = serde_json::from_str(line).ok()?;

    // Ollama streaming: each line has {"message":{"role":"assistant","content":"..."}, "done":false}
    let content = v["message"]["content"].as_str().unwrap_or("").to_string();
    let tool_calls = parse_tool_calls(&v["message"]);
    let done = v["done"].as_bool().unwrap_or(false);

    let usage = if done { parse_usage(&v) } else { None };

    Some(AIMessageChunk {
        content,
        tool_calls,
        usage,
        ..Default::default()
    })
}

#[async_trait]
impl ChatModel for OllamaChatModel {
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

            use futures::StreamExt;

            // NDJSON: accumulate bytes and split on newlines
            let mut buffer = String::new();
            let mut byte_stream = std::pin::pin!(byte_stream);

            while let Some(result) = byte_stream.next().await {
                match result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();
                            if line.is_empty() {
                                continue;
                            }
                            if let Some(chunk) = parse_ndjson_chunk(&line) {
                                yield Ok(chunk);
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                }
            }

            // Process remaining buffer
            let remaining = buffer.trim().to_string();
            if !remaining.is_empty() {
                if let Some(chunk) = parse_ndjson_chunk(&remaining) {
                    yield Ok(chunk);
                }
            }
        })
    }
}
