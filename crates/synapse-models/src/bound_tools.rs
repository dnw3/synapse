use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::{
    ChatModel, ChatRequest, ChatResponse, ChatStream, SynapseError, ToolDefinition,
};

/// A `ChatModel` wrapper that always includes a set of bound tools in every request.
///
/// Created via `BoundToolsChatModel::new(model, tools)`. This is the Rust equivalent
/// of LangChain's `model.bind_tools(tools)`.
pub struct BoundToolsChatModel {
    inner: Arc<dyn ChatModel>,
    tools: Vec<ToolDefinition>,
}

impl BoundToolsChatModel {
    pub fn new(inner: Arc<dyn ChatModel>, tools: Vec<ToolDefinition>) -> Self {
        Self { inner, tools }
    }

    fn inject_tools(&self, mut request: ChatRequest) -> ChatRequest {
        if request.tools.is_empty() {
            request.tools = self.tools.clone();
        } else {
            // Merge: add bound tools that aren't already present
            for tool in &self.tools {
                if !request.tools.iter().any(|t| t.name == tool.name) {
                    request.tools.push(tool.clone());
                }
            }
        }
        request
    }
}

#[async_trait]
impl ChatModel for BoundToolsChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        self.inner.chat(self.inject_tools(request)).await
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        self.inner.stream_chat(self.inject_tools(request))
    }
}
