use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, RunnableConfig, SynapticError};
use synaptic_runnables::{BoxRunnable, Runnable};

/// Wraps a `Runnable<Vec<Message>, String>` with automatic message history
/// load/save from a `MemoryStore`.
///
/// On each invocation:
/// 1. Extracts `session_id` from `config.metadata["session_id"]` (defaults to `"default"`)
/// 2. Loads conversation history from memory
/// 3. Appends `Message::human(input)` to the history
/// 4. Calls the inner runnable with the full message list
/// 5. Appends `Message::ai(output)` to memory
/// 6. Returns the output string
pub struct RunnableWithMessageHistory {
    inner: BoxRunnable<Vec<Message>, String>,
    memory: Arc<dyn MemoryStore>,
}

impl RunnableWithMessageHistory {
    /// Create a new history-aware runnable wrapper.
    pub fn new(inner: BoxRunnable<Vec<Message>, String>, memory: Arc<dyn MemoryStore>) -> Self {
        Self { inner, memory }
    }
}

#[async_trait]
impl Runnable<String, String> for RunnableWithMessageHistory {
    async fn invoke(
        &self,
        input: String,
        config: &RunnableConfig,
    ) -> Result<String, SynapticError> {
        // Extract session_id from config metadata
        let session_id = config
            .metadata
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // Load existing history
        let mut messages = self.memory.load(&session_id).await?;

        // Append the new human message
        let human_msg = Message::human(&input);
        messages.push(human_msg.clone());
        self.memory.append(&session_id, human_msg).await?;

        // Call the inner runnable with the full message list
        let output = self.inner.invoke(messages, config).await?;

        // Save the AI response to memory
        let ai_msg = Message::ai(&output);
        self.memory.append(&session_id, ai_msg).await?;

        Ok(output)
    }
}
