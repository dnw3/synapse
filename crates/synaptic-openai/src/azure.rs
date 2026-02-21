//! Azure OpenAI integration.
//!
//! Azure OpenAI uses a different URL pattern and authentication scheme
//! compared to the standard OpenAI API:
//!
//! - URL: `https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version={version}`
//! - Auth: `api-key: {key}` header (not `Authorization: Bearer`)

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, Embeddings, SynapticError, ToolChoice,
};
use synaptic_models::{ProviderBackend, ProviderRequest};

use crate::chat_model::{
    message_to_openai, parse_response, parse_stream_chunk, tool_def_to_openai,
};
use crate::embeddings::parse_embeddings_response;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for Azure OpenAI chat completions.
#[derive(Debug, Clone)]
pub struct AzureOpenAiConfig {
    pub api_key: String,
    pub resource_name: String,
    pub deployment_name: String,
    pub api_version: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl AzureOpenAiConfig {
    /// Create a new Azure OpenAI config.
    ///
    /// The `deployment_name` typically corresponds to the model you deployed
    /// (e.g. `"gpt-4"`, `"gpt-4o"`).
    pub fn new(
        api_key: impl Into<String>,
        resource_name: impl Into<String>,
        deployment_name: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            resource_name: resource_name.into(),
            deployment_name: deployment_name.into(),
            api_version: "2024-10-21".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
        }
    }

    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
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

// ---------------------------------------------------------------------------
// Chat model
// ---------------------------------------------------------------------------

/// Azure OpenAI chat model.
pub struct AzureOpenAiChatModel {
    config: AzureOpenAiConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl AzureOpenAiChatModel {
    pub fn new(config: AzureOpenAiConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    /// Build a `ProviderRequest` targeting the Azure chat completions endpoint.
    pub fn build_request(&self, request: &ChatRequest, stream: bool) -> ProviderRequest {
        let messages: Vec<Value> = request.messages.iter().map(message_to_openai).collect();

        let mut body = json!({
            "messages": messages,
            "stream": stream,
        });

        if let Some(max_tokens) = self.config.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = self.config.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = self.config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(ref stop) = self.config.stop {
            body["stop"] = json!(stop);
        }
        if !request.tools.is_empty() {
            body["tools"] = json!(request
                .tools
                .iter()
                .map(tool_def_to_openai)
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

        let url = format!(
            "https://{}.openai.azure.com/openai/deployments/{}/chat/completions?api-version={}",
            self.config.resource_name, self.config.deployment_name, self.config.api_version,
        );

        ProviderRequest {
            url,
            headers: vec![
                ("api-key".to_string(), self.config.api_key.clone()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body,
        }
    }
}

#[async_trait]
impl ChatModel for AzureOpenAiChatModel {
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
                        if ev.data == "[DONE]" {
                            break;
                        }
                        if let Some(chunk) = parse_stream_chunk(&ev.data) {
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

// ---------------------------------------------------------------------------
// Embeddings config
// ---------------------------------------------------------------------------

/// Configuration for Azure OpenAI embeddings.
#[derive(Debug, Clone)]
pub struct AzureOpenAiEmbeddingsConfig {
    pub api_key: String,
    pub resource_name: String,
    pub deployment_name: String,
    pub api_version: String,
    pub model: String,
}

impl AzureOpenAiEmbeddingsConfig {
    /// Create a new Azure OpenAI embeddings config.
    pub fn new(
        api_key: impl Into<String>,
        resource_name: impl Into<String>,
        deployment_name: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            resource_name: resource_name.into(),
            deployment_name: deployment_name.into(),
            api_version: "2024-10-21".to_string(),
            model: "text-embedding-3-small".to_string(),
        }
    }

    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

// ---------------------------------------------------------------------------
// Embeddings
// ---------------------------------------------------------------------------

/// Azure OpenAI embeddings.
pub struct AzureOpenAiEmbeddings {
    config: AzureOpenAiEmbeddingsConfig,
    backend: Arc<dyn ProviderBackend>,
}

impl AzureOpenAiEmbeddings {
    pub fn new(config: AzureOpenAiEmbeddingsConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self { config, backend }
    }

    fn build_request(&self, input: Vec<String>) -> ProviderRequest {
        let url = format!(
            "https://{}.openai.azure.com/openai/deployments/{}/embeddings?api-version={}",
            self.config.resource_name, self.config.deployment_name, self.config.api_version,
        );

        ProviderRequest {
            url,
            headers: vec![
                ("api-key".to_string(), self.config.api_key.clone()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: json!({
                "model": self.config.model,
                "input": input,
            }),
        }
    }
}

#[async_trait]
impl Embeddings for AzureOpenAiEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        let input: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let request = self.build_request(input);
        let response = self.backend.send(request).await?;

        if response.status != 200 {
            return Err(SynapticError::Embedding(format!(
                "Azure OpenAI API error ({}): {}",
                response.status, response.body
            )));
        }

        parse_embeddings_response(&response.body)
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let mut results = self.embed_documents(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| SynapticError::Embedding("empty response".to_string()))
    }
}
