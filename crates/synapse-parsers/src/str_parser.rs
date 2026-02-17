use async_trait::async_trait;
use synapse_core::{Message, RunnableConfig, SynapseError};
use synapse_runnables::Runnable;

use crate::FormatInstructions;

/// Extracts the text content from a Message.
pub struct StrOutputParser;

impl FormatInstructions for StrOutputParser {
    fn get_format_instructions(&self) -> String {
        String::new()
    }
}

#[async_trait]
impl Runnable<Message, String> for StrOutputParser {
    async fn invoke(
        &self,
        input: Message,
        _config: &RunnableConfig,
    ) -> Result<String, SynapseError> {
        Ok(input.content().to_string())
    }
}
