use std::collections::HashMap;
use std::sync::Arc;

use super::sender::ChannelSender;

/// Registry of active channel senders.
pub struct ChannelRegistry {
    senders: HashMap<String, Arc<dyn ChannelSender>>,
}

#[allow(dead_code)]
impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
        }
    }

    pub fn register(&mut self, sender: Arc<dyn ChannelSender>) {
        self.senders.insert(sender.channel_id().to_string(), sender);
    }

    pub fn get(&self, channel: &str) -> Option<&Arc<dyn ChannelSender>> {
        self.senders.get(channel)
    }

    pub fn list(&self) -> Vec<&str> {
        self.senders.keys().map(|s| s.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.senders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.senders.is_empty()
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::messages::sender::{ChannelSender, SendResult};
    use async_trait::async_trait;
    use synaptic::DeliveryContext;

    struct MockSender {
        id: String,
    }

    #[async_trait]
    impl ChannelSender for MockSender {
        fn channel_id(&self) -> &str {
            &self.id
        }
        async fn send(
            &self,
            _target: &DeliveryContext,
            _content: &str,
            _meta: Option<&serde_json::Value>,
        ) -> Result<SendResult, Box<dyn std::error::Error + Send + Sync>> {
            Ok(SendResult {
                message_id: Some("test_msg_1".into()),
                delivered_at_ms: 1000,
            })
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = ChannelRegistry::new();
        assert!(reg.is_empty());
        reg.register(Arc::new(MockSender { id: "slack".into() }));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("slack").is_some());
        assert!(reg.get("telegram").is_none());
    }

    #[test]
    fn test_list_channels() {
        let mut reg = ChannelRegistry::new();
        reg.register(Arc::new(MockSender { id: "slack".into() }));
        reg.register(Arc::new(MockSender {
            id: "telegram".into(),
        }));
        let mut list = reg.list();
        list.sort();
        assert_eq!(list, vec!["slack", "telegram"]);
    }

    #[test]
    fn test_register_overwrites() {
        let mut reg = ChannelRegistry::new();
        reg.register(Arc::new(MockSender { id: "slack".into() }));
        reg.register(Arc::new(MockSender { id: "slack".into() }));
        assert_eq!(reg.len(), 1);
    }
}
