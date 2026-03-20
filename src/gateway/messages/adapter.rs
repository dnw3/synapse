use super::outbound::{OutboundDeliveryResult, OutboundPayload};
use async_trait::async_trait;

/// Context for outbound delivery to a specific target.
#[derive(Clone, Debug)]
pub struct OutboundContext {
    pub to: String,
    pub text: String,
    pub media_urls: Vec<String>,
    pub reply_to_id: Option<String>,
    pub thread_id: Option<String>,
    pub account_id: Option<String>,
    pub silent: bool,
}

/// Trait that each channel implements for outbound delivery.
#[async_trait]
pub trait ChannelOutboundAdapter: Send + Sync {
    /// Platform text limit for a single message.
    fn text_chunk_limit(&self) -> usize {
        4000
    }

    /// Split long text into platform-safe chunks.
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let limit = self.text_chunk_limit();
        if text.len() <= limit {
            return vec![text.to_string()];
        }
        text.chars()
            .collect::<Vec<_>>()
            .chunks(limit)
            .map(|c| c.iter().collect::<String>())
            .collect()
    }

    /// Normalize a payload for this platform.
    fn normalize(&self, payload: &OutboundPayload) -> Option<OutboundPayload> {
        Some(payload.clone())
    }

    /// Send a single payload to the target.
    async fn send_payload(
        &self,
        ctx: &OutboundContext,
    ) -> Result<OutboundDeliveryResult, Box<dyn std::error::Error + Send + Sync>>;

    /// Send text content (may be chunked across multiple messages).
    async fn send_text(
        &self,
        ctx: &OutboundContext,
    ) -> Result<Vec<OutboundDeliveryResult>, Box<dyn std::error::Error + Send + Sync>> {
        let chunks = self.chunk_text(&ctx.text);
        let mut results = Vec::new();
        for chunk in chunks {
            let chunk_ctx = OutboundContext {
                text: chunk,
                ..ctx.clone()
            };
            results.push(self.send_payload(&chunk_ctx).await?);
        }
        Ok(results)
    }

    /// Send media content.
    async fn send_media(
        &self,
        ctx: &OutboundContext,
    ) -> Result<OutboundDeliveryResult, Box<dyn std::error::Error + Send + Sync>> {
        self.send_payload(ctx).await
    }
}
