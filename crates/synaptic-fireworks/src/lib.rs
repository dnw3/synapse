use std::sync::Arc;
pub use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapticError};
use synaptic_models::ProviderBackend;
use synaptic_openai::{OpenAiChatModel, OpenAiConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FireworksModel {
    Llama3_1_70bInstruct,
    Llama3_1_8bInstruct,
    DeepSeekR1,
    Qwen2_5_72bInstruct,
    Custom(String),
}

impl FireworksModel {
    pub fn as_str(&self) -> &str {
        match self {
            FireworksModel::Llama3_1_70bInstruct => {
                "accounts/fireworks/models/llama-v3p1-70b-instruct"
            }
            FireworksModel::Llama3_1_8bInstruct => {
                "accounts/fireworks/models/llama-v3p1-8b-instruct"
            }
            FireworksModel::DeepSeekR1 => "accounts/fireworks/models/deepseek-r1",
            FireworksModel::Qwen2_5_72bInstruct => "accounts/fireworks/models/qwen2p5-72b-instruct",
            FireworksModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for FireworksModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct FireworksConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl FireworksConfig {
    pub fn new(api_key: impl Into<String>, model: FireworksModel) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
        }
    }
    pub fn new_custom(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
        }
    }
    pub fn with_max_tokens(mut self, v: u32) -> Self {
        self.max_tokens = Some(v);
        self
    }
    pub fn with_temperature(mut self, v: f64) -> Self {
        self.temperature = Some(v);
        self
    }
    pub fn with_top_p(mut self, v: f64) -> Self {
        self.top_p = Some(v);
        self
    }
    pub fn with_stop(mut self, v: Vec<String>) -> Self {
        self.stop = Some(v);
        self
    }
}

impl From<FireworksConfig> for OpenAiConfig {
    fn from(c: FireworksConfig) -> Self {
        let mut cfg = OpenAiConfig::new(c.api_key, c.model)
            .with_base_url("https://api.fireworks.ai/inference/v1");
        if let Some(v) = c.max_tokens {
            cfg = cfg.with_max_tokens(v);
        }
        if let Some(v) = c.temperature {
            cfg = cfg.with_temperature(v);
        }
        if let Some(v) = c.top_p {
            cfg = cfg.with_top_p(v);
        }
        if let Some(v) = c.stop {
            cfg = cfg.with_stop(v);
        }
        cfg
    }
}

pub struct FireworksChatModel {
    inner: OpenAiChatModel,
}

impl FireworksChatModel {
    pub fn new(config: FireworksConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self {
            inner: OpenAiChatModel::new(config.into(), backend),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for FireworksChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        self.inner.chat(request).await
    }
    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        self.inner.stream_chat(request)
    }
}
