use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};
use synapse_runnables::Runnable;

use crate::FormatInstructions;

/// Separator used to split the input into a list.
#[derive(Debug, Clone)]
pub enum ListSeparator {
    Newline,
    Comma,
    Custom(String),
}

/// Parses a string into a list of strings by splitting on the configured separator.
pub struct ListOutputParser {
    separator: ListSeparator,
}

impl ListOutputParser {
    pub fn new(separator: ListSeparator) -> Self {
        Self { separator }
    }

    /// Creates a parser that splits on newlines (default).
    pub fn newline() -> Self {
        Self::new(ListSeparator::Newline)
    }

    /// Creates a parser that splits on commas.
    pub fn comma() -> Self {
        Self::new(ListSeparator::Comma)
    }
}

impl Default for ListOutputParser {
    fn default() -> Self {
        Self::newline()
    }
}

impl FormatInstructions for ListOutputParser {
    fn get_format_instructions(&self) -> String {
        let sep_desc = match &self.separator {
            ListSeparator::Newline => "newlines",
            ListSeparator::Comma => "commas",
            ListSeparator::Custom(s) => s.as_str(),
        };
        format!("Your response should be a list of items separated by {sep_desc}.")
    }
}

#[async_trait]
impl Runnable<String, Vec<String>> for ListOutputParser {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<Vec<String>, SynapseError> {
        let sep = match &self.separator {
            ListSeparator::Newline => "\n",
            ListSeparator::Comma => ",",
            ListSeparator::Custom(s) => s.as_str(),
        };

        let items: Vec<String> = input
            .split(sep)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(items)
    }
}
