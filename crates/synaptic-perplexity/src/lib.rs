use std::sync::Arc;
pub use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapticError};
use synaptic_models::ProviderBackend;
use synaptic_openai::{OpenAiChatModel, OpenAiConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerplexityModel {
    SonarLarge,
    SonarSmall,
    SonarHuge,
    SonarReasoningPro,
    Custom(String),
}

impl PerplexityModel {
    pub fn as_str(&self) -> &str {
        match self {
            PerplexityModel::SonarLarge => "sonar-large-online",
            PerplexityModel::SonarSmall => "sonar-small-online",
            PerplexityModel::SonarHuge => "sonar-huge-online",
            PerplexityModel::SonarReasoningPro => "sonar-reasoning-pro",
            PerplexityModel::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for PerplexityModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct PerplexityConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

impl PerplexityConfig {
    pub fn new(api_key: impl Into<String>, model: PerplexityModel) -> Self {
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

impl From<PerplexityConfig> for OpenAiConfig {
    fn from(c: PerplexityConfig) -> Self {
        let mut cfg =
            OpenAiConfig::new(c.api_key, c.model).with_base_url("https://api.perplexity.ai");
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

pub struct PerplexityChatModel {
    inner: OpenAiChatModel,
}

impl PerplexityChatModel {
    pub fn new(config: PerplexityConfig, backend: Arc<dyn ProviderBackend>) -> Self {
        Self {
            inner: OpenAiChatModel::new(config.into(), backend),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for PerplexityChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        self.inner.chat(request).await
    }
    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        self.inner.stream_chat(request)
    }
}
