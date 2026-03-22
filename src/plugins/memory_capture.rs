//! MemoryCaptureSubscriber — auto-captures conversation turns to the memory provider.
//!
//! Pattern: EventSubscriber (observation/fire-and-forget), not Interceptor.
//! Also triggers commit() on BeforeCompaction to extract memories before context is lost.

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::events::{Event, EventAction, EventFilter, EventKind, EventSubscriber};
use synaptic::memory::MemoryProvider;

pub struct MemoryCaptureSubscriber {
    provider: Arc<dyn MemoryProvider>,
}

impl MemoryCaptureSubscriber {
    pub fn new(provider: Arc<dyn MemoryProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl EventSubscriber for MemoryCaptureSubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        vec![EventFilter::AnyOf(vec![
            EventKind::MessageReceived,  // Parallel — record user message
            EventKind::AgentEnd,         // Parallel — record assistant response
            EventKind::BeforeCompaction, // Sequential — commit before compact
        ])]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        let session_key = event
            .payload
            .get("session_key")
            .or_else(|| event.payload.get("sessionKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match event.kind {
            EventKind::MessageReceived => {
                if let Some(content) = event.payload.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        if let Err(e) = self
                            .provider
                            .add_message(session_key, "user", content)
                            .await
                        {
                            tracing::warn!(error = %e, "failed to capture user message");
                        }
                    }
                }
            }
            EventKind::AgentEnd => {
                if let Some(content) = event.payload.get("response").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        if let Err(e) = self
                            .provider
                            .add_message(session_key, "assistant", content)
                            .await
                        {
                            tracing::warn!(error = %e, "failed to capture assistant message");
                        }
                    }
                }
            }
            EventKind::BeforeCompaction => match self.provider.commit(session_key).await {
                Ok(result) => {
                    tracing::info!(
                        session = session_key,
                        extracted = result.memories_extracted,
                        merged = result.memories_merged,
                        "committed memories before compaction"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "memory commit before compaction failed");
                }
            },
            _ => {}
        }

        Ok(EventAction::Continue)
    }

    fn name(&self) -> &str {
        "MemoryCaptureSubscriber"
    }
}
