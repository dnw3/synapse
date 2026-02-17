use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapseError};

/// Wraps a ChatModel to produce structured JSON output.
///
/// Injects a system prompt instructing the model to respond with valid JSON
/// matching a given schema description, then parses the response.
pub struct StructuredOutputChatModel<T> {
    inner: Arc<dyn ChatModel>,
    schema_description: String,
    _marker: PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> StructuredOutputChatModel<T> {
    /// Create a new StructuredOutputChatModel.
    ///
    /// `schema_description` should describe the expected JSON shape, e.g.:
    /// `{"name": "string", "age": "number", "tags": ["string"]}`
    pub fn new(inner: Arc<dyn ChatModel>, schema_description: impl Into<String>) -> Self {
        Self {
            inner,
            schema_description: schema_description.into(),
            _marker: PhantomData,
        }
    }

    /// Parse the model's text response as JSON into type T.
    pub fn parse_response(&self, response: &ChatResponse) -> Result<T, SynapseError> {
        let text = response.message.content();
        // Try to extract JSON from the response -- handle markdown code blocks
        let json_str = extract_json(text);
        serde_json::from_str::<T>(json_str)
            .map_err(|e| SynapseError::Parsing(format!("failed to parse structured output: {e}")))
    }

    /// Call the model and parse the response as T.
    pub async fn generate(&self, request: ChatRequest) -> Result<(T, ChatResponse), SynapseError> {
        let response = self.chat(request).await?;
        let parsed = self.parse_response(&response)?;
        Ok((parsed, response))
    }
}

/// Extract JSON from text, handling optional markdown code blocks.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    // Check for ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7; // skip "```json"
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    // Check for ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    trimmed
}

#[async_trait]
impl<T: DeserializeOwned + Send + Sync + 'static> ChatModel for StructuredOutputChatModel<T> {
    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        // Inject system message with schema instructions
        let instruction = format!(
            "You MUST respond with valid JSON matching this schema:\n{}\n\nDo not include any text outside the JSON object. Do not use markdown code blocks.",
            self.schema_description
        );

        // Prepend system message
        request.messages.insert(0, Message::system(instruction));

        self.inner.chat(request).await
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        // Streaming delegates to inner (structured output parsing happens after collection)
        self.inner.stream_chat(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_plain() {
        assert_eq!(extract_json(r#"{"a": 1}"#), r#"{"a": 1}"#);
    }

    #[test]
    fn extract_json_code_block() {
        let input = "```json\n{\"a\": 1}\n```";
        assert_eq!(extract_json(input), r#"{"a": 1}"#);
    }

    #[test]
    fn extract_json_plain_code_block() {
        let input = "```\n{\"a\": 1}\n```";
        assert_eq!(extract_json(input), r#"{"a": 1}"#);
    }

    #[test]
    fn extract_json_with_surrounding_whitespace() {
        assert_eq!(extract_json("  {\"a\": 1}  "), r#"{"a": 1}"#);
    }
}
