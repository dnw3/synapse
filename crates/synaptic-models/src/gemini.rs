use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapseError,
    TokenUsage, ToolCall, ToolChoice, ToolDefinition,
};

use crate::backend::{ProviderBackend, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
pub struct GeminiConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl GeminiConfig {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            top_p: None,
            stop: None,
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
}

pub struct GeminiChatModel {
    config: GeminiConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl GeminiChatModel {
    pub fn new(config: GeminiConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    fn build_request(&self, request: &ChatRequest, stream: bool) -> ProviderRequest {
        let mut system_text: Option<String> = None;
        let mut contents: Vec<Value> = Vec::new();

        for msg in &request.messages {
            match msg {
                Message::System { content, .. } => {
                    system_text = Some(content.clone());
                }
                Message::Human { content, .. } => {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{"text": content}],
                    }));
                }
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut parts: Vec<Value> = Vec::new();
                    if !content.is_empty() {
                        parts.push(json!({"text": content}));
                    }
                    for tc in tool_calls {
                        parts.push(json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments,
                            }
                        }));
                    }
                    contents.push(json!({
                        "role": "model",
                        "parts": parts,
                    }));
                }
                Message::Tool {
                    content,
                    tool_call_id: _,
                    ..
                } => {
                    // Gemini uses functionResponse in parts
                    let result: Value =
                        serde_json::from_str(content).unwrap_or(json!({"result": content}));
                    contents.push(json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": "tool",
                                "response": result,
                            }
                        }],
                    }));
                }
                Message::Chat { content, .. } => {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{"text": content}],
                    }));
                }
                Message::Remove { .. } => { /* skip Remove messages */ }
            }
        }

        let mut body = json!({
            "contents": contents,
        });

        if let Some(system) = system_text {
            body["system_instruction"] = json!({
                "parts": [{"text": system}],
            });
        }

        {
            let mut gen_config = json!({});
            let mut has_gen_config = false;
            if let Some(top_p) = self.config.top_p {
                gen_config["topP"] = json!(top_p);
                has_gen_config = true;
            }
            if let Some(ref stop) = self.config.stop {
                gen_config["stopSequences"] = json!(stop);
                has_gen_config = true;
            }
            if has_gen_config {
                body["generationConfig"] = gen_config;
            }
        }

        if !request.tools.is_empty() {
            body["tools"] = json!([{
                "functionDeclarations": request.tools.iter().map(tool_def_to_gemini).collect::<Vec<_>>(),
            }]);
        }
        if let Some(ref choice) = request.tool_choice {
            body["tool_config"] = match choice {
                ToolChoice::Auto => json!({"functionCallingConfig": {"mode": "AUTO"}}),
                ToolChoice::Required => json!({"functionCallingConfig": {"mode": "ANY"}}),
                ToolChoice::None => json!({"functionCallingConfig": {"mode": "NONE"}}),
                ToolChoice::Specific(name) => json!({
                    "functionCallingConfig": {
                        "mode": "ANY",
                        "allowedFunctionNames": [name]
                    }
                }),
            };
        }

        let method = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };

        let mut url = format!(
            "{}/v1beta/models/{}:{}?key={}",
            self.config.base_url, self.config.model, method, self.config.api_key
        );
        if stream {
            url.push_str("&alt=sse");
        }

        ProviderRequest {
            url,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body,
        }
    }
}

fn tool_def_to_gemini(def: &ToolDefinition) -> Value {
    json!({
        "name": def.name,
        "description": def.description,
        "parameters": def.parameters,
    })
}

fn parse_response(resp: &ProviderResponse) -> Result<ChatResponse, SynapseError> {
    check_error_status(resp)?;

    let parts = resp.body["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for part in &parts {
        if let Some(t) = part["text"].as_str() {
            text.push_str(t);
        }
        if let Some(fc) = part.get("functionCall") {
            if let Some(name) = fc["name"].as_str() {
                tool_calls.push(ToolCall {
                    id: format!("gemini-{}", tool_calls.len()),
                    name: name.to_string(),
                    arguments: fc["args"].clone(),
                });
            }
        }
    }

    let usage = parse_usage(&resp.body["usageMetadata"]);

    let message = if tool_calls.is_empty() {
        Message::ai(text)
    } else {
        Message::ai_with_tool_calls(text, tool_calls)
    };

    Ok(ChatResponse { message, usage })
}

fn check_error_status(resp: &ProviderResponse) -> Result<(), SynapseError> {
    if resp.status == 429 {
        let msg = resp.body["error"]["message"]
            .as_str()
            .unwrap_or("rate limited")
            .to_string();
        return Err(SynapseError::RateLimit(msg));
    }
    if resp.status >= 400 {
        let msg = resp.body["error"]["message"]
            .as_str()
            .unwrap_or("unknown API error")
            .to_string();
        return Err(SynapseError::Model(format!(
            "Gemini API error ({}): {}",
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
        input_tokens: usage["promptTokenCount"].as_u64().unwrap_or(0) as u32,
        output_tokens: usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
        total_tokens: usage["totalTokenCount"].as_u64().unwrap_or(0) as u32,
        input_details: None,
        output_details: None,
    })
}

fn parse_stream_chunk(data: &str) -> Option<AIMessageChunk> {
    let v: Value = serde_json::from_str(data).ok()?;
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut content = String::new();
    let mut tool_calls = Vec::new();

    for part in &parts {
        if let Some(t) = part["text"].as_str() {
            content.push_str(t);
        }
        if let Some(fc) = part.get("functionCall") {
            if let Some(name) = fc["name"].as_str() {
                tool_calls.push(ToolCall {
                    id: format!("gemini-{}", tool_calls.len()),
                    name: name.to_string(),
                    arguments: fc["args"].clone(),
                });
            }
        }
    }

    let usage = parse_usage(&v["usageMetadata"]);

    Some(AIMessageChunk {
        content,
        tool_calls,
        usage,
        ..Default::default()
    })
}

#[async_trait]
impl ChatModel for GeminiChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
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
                        if let Some(chunk) = parse_stream_chunk(&ev.data) {
                            yield Ok(chunk);
                        }
                    }
                    Err(e) => {
                        yield Err(SynapseError::Model(format!("SSE parse error: {e}")));
                        break;
                    }
                }
            }
        })
    }
}
