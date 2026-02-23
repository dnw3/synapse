use crate::Message;

/// Trait for counting tokens in text and messages.
pub trait TokenCounter: Send + Sync {
    /// Count the number of tokens in a text string.
    fn count_text(&self, text: &str) -> usize;

    /// Count the total number of tokens in a slice of messages.
    /// Default implementation sums count_text(content) + 4 per message overhead.
    fn count_messages(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| self.count_text(m.content()) + 4)
            .sum()
    }
}

/// Heuristic token counter that estimates ~4 characters per token.
pub struct HeuristicTokenCounter;

impl TokenCounter for HeuristicTokenCounter {
    fn count_text(&self, text: &str) -> usize {
        // ~4 chars per token, minimum 1 token for non-empty text
        let count = text.len() / 4;
        if text.is_empty() {
            0
        } else {
            count.max(1)
        }
    }
}
