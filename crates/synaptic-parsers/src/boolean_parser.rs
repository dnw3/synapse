use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::FormatInstructions;

/// Parses yes/no, true/false, y/n style responses into a boolean.
pub struct BooleanOutputParser;

impl FormatInstructions for BooleanOutputParser {
    fn get_format_instructions(&self) -> String {
        "Your response should be a boolean value: true/false, yes/no, y/n, or 1/0.".to_string()
    }
}

#[async_trait]
impl Runnable<String, bool> for BooleanOutputParser {
    async fn invoke(&self, input: String, _config: &RunnableConfig) -> Result<bool, SynapseError> {
        let normalized = input.trim().to_lowercase();

        match normalized.as_str() {
            "true" | "yes" | "y" | "1" => Ok(true),
            "false" | "no" | "n" | "0" => Ok(false),
            _ => Err(SynapseError::Parsing(format!(
                "cannot parse '{normalized}' as boolean; expected one of: true, false, yes, no, y, n, 1, 0"
            ))),
        }
    }
}
