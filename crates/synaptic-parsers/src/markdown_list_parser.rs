use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::FormatInstructions;

/// Parses markdown-formatted lists (both `- item` and `* item`).
pub struct MarkdownListOutputParser;

impl FormatInstructions for MarkdownListOutputParser {
    fn get_format_instructions(&self) -> String {
        "Your response should be a markdown list using `- ` or `* ` for each item.".to_string()
    }
}

#[async_trait]
impl Runnable<String, Vec<String>> for MarkdownListOutputParser {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<Vec<String>, SynapseError> {
        let items: Vec<String> = input
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("- ") {
                    let item = rest.trim().to_string();
                    if item.is_empty() {
                        None
                    } else {
                        Some(item)
                    }
                } else if let Some(rest) = trimmed.strip_prefix("* ") {
                    let item = rest.trim().to_string();
                    if item.is_empty() {
                        None
                    } else {
                        Some(item)
                    }
                } else {
                    None
                }
            })
            .collect();

        Ok(items)
    }
}
