use std::sync::Arc;

use crate::Condenser;
use async_trait::async_trait;
use synaptic_core::{Message, SynapticError, TokenCounter};

/// Trims messages to fit within a token budget, keeping the most recent messages.
pub struct TokenBudgetCondenser {
    max_tokens: usize,
    counter: Arc<dyn TokenCounter>,
    include_system: bool,
}

impl TokenBudgetCondenser {
    pub fn new(max_tokens: usize, counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            max_tokens,
            counter,
            include_system: true,
        }
    }

    pub fn with_include_system(mut self, include: bool) -> Self {
        self.include_system = include;
        self
    }
}

#[async_trait]
impl Condenser for TokenBudgetCondenser {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError> {
        let total = self.counter.count_messages(&messages);
        if total <= self.max_tokens {
            return Ok(messages);
        }

        // Preserve system message if configured
        let (system_msg, rest) =
            if self.include_system && !messages.is_empty() && messages[0].is_system() {
                (Some(messages[0].clone()), &messages[1..])
            } else {
                (None, messages.as_slice())
            };

        let system_tokens = system_msg
            .as_ref()
            .map(|m| self.counter.count_messages(std::slice::from_ref(m)))
            .unwrap_or(0);
        let budget = self.max_tokens.saturating_sub(system_tokens);

        // Keep most recent messages that fit
        let mut kept = Vec::new();
        let mut used = 0;
        for msg in rest.iter().rev() {
            let tokens = self.counter.count_messages(std::slice::from_ref(msg));
            if used + tokens > budget {
                break;
            }
            used += tokens;
            kept.push(msg.clone());
        }
        kept.reverse();

        let mut result = Vec::new();
        if let Some(sys) = system_msg {
            result.push(sys);
        }
        result.extend(kept);
        Ok(result)
    }
}
