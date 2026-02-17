use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};
use synapse_runnables::Runnable;

use crate::FormatInstructions;

/// Parses numbered lists like `1. item`, `2. item`.
pub struct NumberedListOutputParser;

impl FormatInstructions for NumberedListOutputParser {
    fn get_format_instructions(&self) -> String {
        "Your response should be a numbered list (e.g., `1. item`, `2. item`).".to_string()
    }
}

#[async_trait]
impl Runnable<String, Vec<String>> for NumberedListOutputParser {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<Vec<String>, SynapseError> {
        let items: Vec<String> = input
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                // Find the first '.' and check that everything before it is digits
                let dot_pos = trimmed.find('.')?;
                let prefix = &trimmed[..dot_pos];
                if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                // After the dot, expect at least one whitespace char
                let after_dot = &trimmed[dot_pos + 1..];
                let rest = after_dot
                    .strip_prefix(' ')
                    .or_else(|| after_dot.strip_prefix('\t'))?;
                let item = rest.trim().to_string();
                if item.is_empty() {
                    None
                } else {
                    Some(item)
                }
            })
            .collect();

        Ok(items)
    }
}
