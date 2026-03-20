use async_trait::async_trait;
use synaptic::DeliveryContext;

/// Result of sending a message to a channel.
#[allow(dead_code)]
pub struct SendResult {
    /// Platform-returned message ID (for future editing/threading).
    pub message_id: Option<String>,
    pub delivered_at_ms: u64,
}

/// Trait for sending outbound messages to a specific channel.
#[async_trait]
#[allow(dead_code)]
pub trait ChannelSender: Send + Sync {
    /// Channel identifier this sender handles (e.g. "slack", "telegram").
    fn channel_id(&self) -> &str;

    /// Send a message to the given delivery target.
    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        meta: Option<&serde_json::Value>,
    ) -> Result<SendResult, Box<dyn std::error::Error + Send + Sync>>;
}
