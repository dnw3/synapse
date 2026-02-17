use std::{collections::VecDeque, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use synaptic_core::SynapseError;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub status: u16,
    pub body: Value,
}

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<bytes::Bytes, SynapseError>> + Send>>;

#[async_trait]
pub trait ProviderBackend: Send + Sync {
    async fn send(&self, request: ProviderRequest) -> Result<ProviderResponse, SynapseError>;
    async fn send_stream(&self, request: ProviderRequest) -> Result<ByteStream, SynapseError>;
}

/// Production backend using reqwest.
pub struct HttpBackend {
    client: reqwest::Client,
}

impl HttpBackend {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for HttpBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderBackend for HttpBackend {
    async fn send(&self, request: ProviderRequest) -> Result<ProviderResponse, SynapseError> {
        let mut builder = self.client.post(&request.url);
        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        builder = builder.json(&request.body);

        let response = builder
            .send()
            .await
            .map_err(|e| SynapseError::Model(format!("HTTP request failed: {e}")))?;

        let status = response.status().as_u16();
        let body: Value = response
            .json()
            .await
            .map_err(|e| SynapseError::Parsing(format!("failed to parse response JSON: {e}")))?;

        Ok(ProviderResponse { status, body })
    }

    async fn send_stream(&self, request: ProviderRequest) -> Result<ByteStream, SynapseError> {
        use futures::StreamExt;

        let mut builder = self.client.post(&request.url);
        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        builder = builder.json(&request.body);

        let response = builder
            .send()
            .await
            .map_err(|e| SynapseError::Model(format!("HTTP stream request failed: {e}")))?;

        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(|e| SynapseError::Model(format!("stream error: {e}"))));

        Ok(Box::pin(stream))
    }
}

/// Test backend with queued responses and stream chunks.
pub struct FakeBackend {
    responses: Arc<Mutex<VecDeque<Result<ProviderResponse, SynapseError>>>>,
    stream_chunks: Arc<Mutex<VecDeque<Vec<bytes::Bytes>>>>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            stream_chunks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push_response(&self, response: ProviderResponse) -> &Self {
        self.responses
            .try_lock()
            .expect("not concurrent during setup")
            .push_back(Ok(response));
        self
    }

    pub fn push_error(&self, error: SynapseError) -> &Self {
        self.responses
            .try_lock()
            .expect("not concurrent during setup")
            .push_back(Err(error));
        self
    }

    pub fn push_stream_chunks(&self, chunks: Vec<bytes::Bytes>) -> &Self {
        self.stream_chunks
            .try_lock()
            .expect("not concurrent during setup")
            .push_back(chunks);
        self
    }
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderBackend for FakeBackend {
    async fn send(&self, _request: ProviderRequest) -> Result<ProviderResponse, SynapseError> {
        let mut responses = self.responses.lock().await;
        responses
            .pop_front()
            .unwrap_or_else(|| Err(SynapseError::Model("FakeBackend exhausted".to_string())))
    }

    async fn send_stream(&self, _request: ProviderRequest) -> Result<ByteStream, SynapseError> {
        let mut stream_chunks = self.stream_chunks.lock().await;
        let chunks = stream_chunks.pop_front().unwrap_or_default();

        let stream = futures::stream::iter(chunks.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}
