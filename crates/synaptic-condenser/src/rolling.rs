use crate::Condenser;
use async_trait::async_trait;
use synaptic_core::{Message, SynapticError};

/// Keeps only the most recent N messages, optionally preserving the system message.
pub struct RollingCondenser {
    pub max_messages: usize,
    pub preserve_system: bool,
}

impl RollingCondenser {
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            preserve_system: true,
        }
    }

    pub fn with_preserve_system(mut self, preserve: bool) -> Self {
        self.preserve_system = preserve;
        self
    }
}

#[async_trait]
impl Condenser for RollingCondenser {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError> {
        if messages.len() <= self.max_messages {
            return Ok(messages);
        }

        if self.preserve_system && !messages.is_empty() && messages[0].is_system() {
            let system = messages[0].clone();
            let rest = &messages[1..];
            let keep = if self.max_messages > 0 {
                self.max_messages - 1
            } else {
                0
            };
            let start = rest.len().saturating_sub(keep);
            let mut result = vec![system];
            result.extend_from_slice(&rest[start..]);
            Ok(result)
        } else {
            let start = messages.len().saturating_sub(self.max_messages);
            Ok(messages[start..].to_vec())
        }
    }
}
