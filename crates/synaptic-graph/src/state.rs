use serde::{Deserialize, Serialize};
use synaptic_core::Message;

/// Trait for graph state. Types implementing this can be used as graph state.
pub trait State: Clone + Send + Sync + 'static {
    /// Merge another state into this one (reducer pattern).
    fn merge(&mut self, other: Self);
}

/// Built-in state containing a list of messages (most common use case).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageState {
    pub messages: Vec<Message>,
}

impl MessageState {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }

    pub fn with_messages(messages: Vec<Message>) -> Self {
        Self { messages }
    }

    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }
}

impl State for MessageState {
    fn merge(&mut self, other: Self) {
        self.messages.extend(other.messages);
    }
}
