use std::marker::PhantomData;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::FormatInstructions;

/// Parses a JSON string and deserializes it into type `T`.
pub struct StructuredOutputParser<T> {
    _phantom: PhantomData<T>,
}

impl<T> StructuredOutputParser<T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for StructuredOutputParser<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FormatInstructions for StructuredOutputParser<T> {
    fn get_format_instructions(&self) -> String {
        "Your response should be a valid JSON object matching the expected schema.".to_string()
    }
}

#[async_trait]
impl<T> Runnable<String, T> for StructuredOutputParser<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    async fn invoke(&self, input: String, _config: &RunnableConfig) -> Result<T, SynapseError> {
        serde_json::from_str(&input)
            .map_err(|e| SynapseError::Parsing(format!("structured parse error: {e}")))
    }
}
