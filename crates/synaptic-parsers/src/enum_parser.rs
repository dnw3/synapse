use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::FormatInstructions;

/// Validates that the trimmed input matches one of the allowed enum values.
pub struct EnumOutputParser {
    allowed: Vec<String>,
}

impl EnumOutputParser {
    pub fn new(allowed: Vec<String>) -> Self {
        Self { allowed }
    }
}

impl FormatInstructions for EnumOutputParser {
    fn get_format_instructions(&self) -> String {
        let values = self.allowed.join(", ");
        format!("Your response should be one of the following values: {values}")
    }
}

#[async_trait]
impl Runnable<String, String> for EnumOutputParser {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<String, SynapseError> {
        let trimmed = input.trim().to_string();
        if self.allowed.contains(&trimmed) {
            Ok(trimmed)
        } else {
            Err(SynapseError::Parsing(format!(
                "expected one of {:?}, got '{trimmed}'",
                self.allowed
            )))
        }
    }
}
