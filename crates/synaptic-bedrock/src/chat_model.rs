use std::collections::HashMap;

use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{
    self as bedrock_types, ContentBlock, ConversationRole, InferenceConfiguration,
    SystemContentBlock, ToolConfiguration, ToolInputSchema, ToolResultBlock,
    ToolResultContentBlock, ToolSpecification, ToolUseBlock,
};
use aws_smithy_types::Document as SmithyDocument;
use serde_json::Value;
use synaptic_core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapticError,
    TokenUsage, ToolCall, ToolCallChunk, ToolChoice,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the AWS Bedrock chat model.
#[derive(Debug, Clone)]
pub struct BedrockConfig {
    /// The model identifier (e.g., `"anthropic.claude-3-5-sonnet-20241022-v2:0"`).
    pub model_id: String,
    /// AWS region override. Falls back to `AWS_REGION` env var or `"us-east-1"`.
    pub region: Option<String>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<i32>,
    /// Sampling temperature (0.0 - 1.0).
    pub temperature: Option<f32>,
    /// Nucleus sampling parameter.
    pub top_p: Option<f32>,
    /// Stop sequences.
    pub stop: Option<Vec<String>>,
}

impl BedrockConfig {
    /// Create a new configuration with the given model ID.
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            region: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
        }
    }

    /// Set the AWS region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set the maximum number of output tokens.
    pub fn with_max_tokens(mut self, max_tokens: i32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the sampling temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the nucleus sampling parameter.
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set stop sequences.
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }
}

// ---------------------------------------------------------------------------
// BedrockChatModel
// ---------------------------------------------------------------------------

/// A [`ChatModel`] implementation backed by AWS Bedrock's Converse API.
///
/// Supports both synchronous and streaming responses, tool calling,
/// and all Bedrock-supported foundation models.
pub struct BedrockChatModel {
    config: BedrockConfig,
    client: aws_sdk_bedrockruntime::Client,
}

impl BedrockChatModel {
    /// Create a new `BedrockChatModel` by loading AWS configuration from the
    /// environment. Respects `AWS_REGION`, `AWS_ACCESS_KEY_ID`,
    /// `AWS_SECRET_ACCESS_KEY`, and other standard AWS SDK environment variables.
    pub async fn new(config: BedrockConfig) -> Self {
        let mut aws_config_loader = aws_config::from_env();

        if let Some(ref region) = config.region {
            aws_config_loader =
                aws_config_loader.region(aws_config::Region::new(region.clone()));
        }

        let aws_config = aws_config_loader.load().await;
        let client = aws_sdk_bedrockruntime::Client::new(&aws_config);

        Self { config, client }
    }

    /// Create a new `BedrockChatModel` with a pre-existing AWS SDK client.
    pub fn from_client(config: BedrockConfig, client: aws_sdk_bedrockruntime::Client) -> Self {
        Self { config, client }
    }

    /// Build the inference configuration from our config.
    fn build_inference_config(&self) -> Option<InferenceConfiguration> {
        let has_any = self.config.max_tokens.is_some()
            || self.config.temperature.is_some()
            || self.config.top_p.is_some()
            || self.config.stop.is_some();

        if !has_any {
            return None;
        }

        let mut builder = InferenceConfiguration::builder();

        if let Some(max_tokens) = self.config.max_tokens {
            builder = builder.max_tokens(max_tokens);
        }
        if let Some(temperature) = self.config.temperature {
            builder = builder.temperature(temperature);
        }
        if let Some(top_p) = self.config.top_p {
            builder = builder.top_p(top_p);
        }
        if let Some(ref stop) = self.config.stop {
            for s in stop {
                builder = builder.stop_sequences(s.clone());
            }
        }

        Some(builder.build())
    }

    /// Build the tool configuration from a ChatRequest.
    fn build_tool_config(
        &self,
        request: &ChatRequest,
    ) -> Option<ToolConfiguration> {
        if request.tools.is_empty() {
            return None;
        }

        let tools: Vec<bedrock_types::Tool> = request
            .tools
            .iter()
            .map(|td| {
                let spec = ToolSpecification::builder()
                    .name(&td.name)
                    .description(&td.description)
                    .input_schema(ToolInputSchema::Json(json_value_to_document(&td.parameters)))
                    .build()
                    .expect("tool specification build should not fail");

                bedrock_types::Tool::ToolSpec(spec)
            })
            .collect();

        let mut builder = ToolConfiguration::builder();
        for tool in tools {
            builder = builder.tools(tool);
        }

        if let Some(ref choice) = request.tool_choice {
            let bedrock_choice = match choice {
                ToolChoice::Auto => {
                    bedrock_types::ToolChoice::Auto(
                        bedrock_types::AutoToolChoice::builder().build(),
                    )
                }
                ToolChoice::Required => {
                    bedrock_types::ToolChoice::Any(
                        bedrock_types::AnyToolChoice::builder().build(),
                    )
                }
                ToolChoice::None => {
                    // Bedrock does not have a "none" tool choice. We omit tools instead,
                    // but since we already built them, just default to Auto.
                    bedrock_types::ToolChoice::Auto(
                        bedrock_types::AutoToolChoice::builder().build(),
                    )
                }
                ToolChoice::Specific(name) => {
                    bedrock_types::ToolChoice::Tool(
                        bedrock_types::SpecificToolChoice::builder()
                            .name(name)
                            .build()
                            .expect("specific tool choice build should not fail"),
                    )
                }
            };
            builder = builder.tool_choice(bedrock_choice);
        }

        Some(builder.build().expect("tool configuration build should not fail"))
    }
}

#[async_trait]
impl ChatModel for BedrockChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let (system_blocks, messages) = convert_messages(&request.messages);

        let mut converse = self
            .client
            .converse()
            .model_id(&self.config.model_id);

        // Add system prompts.
        for block in system_blocks {
            converse = converse.system(block);
        }

        // Add messages.
        for msg in messages {
            converse = converse.messages(msg);
        }

        // Add inference config.
        if let Some(inference_config) = self.build_inference_config() {
            converse = converse.inference_config(inference_config);
        }

        // Add tool config.
        if let Some(tool_config) = self.build_tool_config(&request) {
            converse = converse.tool_config(tool_config);
        }

        let output = converse
            .send()
            .await
            .map_err(|e| SynapticError::Model(format!("Bedrock Converse API error: {e}")))?;

        // Parse usage.
        let usage = output.usage().map(|u| TokenUsage {
            input_tokens: u.input_tokens() as u32,
            output_tokens: u.output_tokens() as u32,
            total_tokens: u.total_tokens() as u32,
            input_details: None,
            output_details: None,
        });

        // Parse the output message.
        let message = match output.output() {
            Some(bedrock_types::ConverseOutput::Message(msg)) => {
                parse_bedrock_message(msg)
            }
            _ => Message::ai(""),
        };

        Ok(ChatResponse { message, usage })
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        Box::pin(async_stream::stream! {
            let (system_blocks, messages) = convert_messages(&request.messages);

            let mut converse_stream = self
                .client
                .converse_stream()
                .model_id(&self.config.model_id);

            for block in system_blocks {
                converse_stream = converse_stream.system(block);
            }

            for msg in messages {
                converse_stream = converse_stream.messages(msg);
            }

            if let Some(inference_config) = self.build_inference_config() {
                converse_stream = converse_stream.inference_config(inference_config);
            }

            if let Some(tool_config) = self.build_tool_config(&request) {
                converse_stream = converse_stream.tool_config(tool_config);
            }

            let output = match converse_stream.send().await {
                Ok(o) => o,
                Err(e) => {
                    yield Err(SynapticError::Model(format!(
                        "Bedrock ConverseStream API error: {e}"
                    )));
                    return;
                }
            };

            let mut stream = output.stream;

            // Track current tool use blocks being built during streaming.
            let mut current_tool_id: Option<String> = None;
            let mut current_tool_name: Option<String> = None;
            let mut current_tool_input: String = String::new();

            loop {
                match stream.recv().await {
                    Ok(Some(event)) => {
                        match event {
                            bedrock_types::ConverseStreamOutput::ContentBlockStart(start_event) => {
                                if let Some(bedrock_types::ContentBlockStart::ToolUse(tool_start)) = start_event.start() {
                                    current_tool_id = Some(tool_start.tool_use_id().to_string());
                                    current_tool_name = Some(tool_start.name().to_string());
                                    current_tool_input.clear();

                                    yield Ok(AIMessageChunk {
                                        tool_call_chunks: vec![ToolCallChunk {
                                            id: Some(tool_start.tool_use_id().to_string()),
                                            name: Some(tool_start.name().to_string()),
                                            arguments: None,
                                            index: Some(start_event.content_block_index() as usize),
                                        }],
                                        ..Default::default()
                                    });
                                }
                            }
                            bedrock_types::ConverseStreamOutput::ContentBlockDelta(delta_event) => {
                                if let Some(delta) = delta_event.delta() {
                                    match delta {
                                        bedrock_types::ContentBlockDelta::Text(text) => {
                                            yield Ok(AIMessageChunk {
                                                content: text.to_string(),
                                                ..Default::default()
                                            });
                                        }
                                        bedrock_types::ContentBlockDelta::ToolUse(tool_delta) => {
                                            let input_fragment = tool_delta.input();
                                            current_tool_input.push_str(input_fragment);

                                            yield Ok(AIMessageChunk {
                                                tool_call_chunks: vec![ToolCallChunk {
                                                    id: current_tool_id.clone(),
                                                    name: current_tool_name.clone(),
                                                    arguments: Some(input_fragment.to_string()),
                                                    index: Some(delta_event.content_block_index() as usize),
                                                }],
                                                ..Default::default()
                                            });
                                        }
                                        _ => { /* ignore other delta types */ }
                                    }
                                }
                            }
                            bedrock_types::ConverseStreamOutput::ContentBlockStop(_) => {
                                // If we were accumulating a tool call, emit the complete ToolCall.
                                if let (Some(id), Some(name)) = (current_tool_id.take(), current_tool_name.take()) {
                                    let arguments: Value = serde_json::from_str(&current_tool_input)
                                        .unwrap_or(Value::Object(Default::default()));
                                    current_tool_input.clear();

                                    yield Ok(AIMessageChunk {
                                        tool_calls: vec![ToolCall {
                                            id,
                                            name,
                                            arguments,
                                        }],
                                        ..Default::default()
                                    });
                                }
                            }
                            bedrock_types::ConverseStreamOutput::Metadata(meta) => {
                                if let Some(u) = meta.usage() {
                                    yield Ok(AIMessageChunk {
                                        usage: Some(TokenUsage {
                                            input_tokens: u.input_tokens() as u32,
                                            output_tokens: u.output_tokens() as u32,
                                            total_tokens: u.total_tokens() as u32,
                                            input_details: None,
                                            output_details: None,
                                        }),
                                        ..Default::default()
                                    });
                                }
                            }
                            _ => { /* MessageStart, MessageStop, Unknown — skip */ }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(SynapticError::Model(format!(
                            "Bedrock stream error: {e}"
                        )));
                        break;
                    }
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Message conversion helpers
// ---------------------------------------------------------------------------

/// Convert Synaptic messages into Bedrock system blocks and conversation messages.
///
/// System messages are extracted into `SystemContentBlock` entries.
/// Human, AI, and Tool messages are mapped to Bedrock `Message` types.
fn convert_messages(
    messages: &[Message],
) -> (Vec<SystemContentBlock>, Vec<bedrock_types::Message>) {
    let mut system_blocks = Vec::new();
    let mut bedrock_messages: Vec<bedrock_types::Message> = Vec::new();

    for msg in messages {
        match msg {
            Message::System { content, .. } => {
                system_blocks.push(SystemContentBlock::Text(content.clone()));
            }
            Message::Human { content, .. } => {
                let bedrock_msg = bedrock_types::Message::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(content.clone()))
                    .build()
                    .expect("message build should not fail");
                bedrock_messages.push(bedrock_msg);
            }
            Message::AI {
                content,
                tool_calls,
                ..
            } => {
                let mut blocks: Vec<ContentBlock> = Vec::new();

                if !content.is_empty() {
                    blocks.push(ContentBlock::Text(content.clone()));
                }

                for tc in tool_calls {
                    let tool_use = ToolUseBlock::builder()
                        .tool_use_id(&tc.id)
                        .name(&tc.name)
                        .input(json_value_to_document(&tc.arguments))
                        .build()
                        .expect("tool use block build should not fail");
                    blocks.push(ContentBlock::ToolUse(tool_use));
                }

                // Bedrock requires at least one content block.
                if blocks.is_empty() {
                    blocks.push(ContentBlock::Text(String::new()));
                }

                let bedrock_msg = bedrock_types::Message::builder()
                    .role(ConversationRole::Assistant)
                    .set_content(Some(blocks))
                    .build()
                    .expect("message build should not fail");
                bedrock_messages.push(bedrock_msg);
            }
            Message::Tool {
                content,
                tool_call_id,
                ..
            } => {
                let tool_result = ToolResultBlock::builder()
                    .tool_use_id(tool_call_id)
                    .content(ToolResultContentBlock::Text(content.clone()))
                    .build()
                    .expect("tool result block build should not fail");

                let bedrock_msg = bedrock_types::Message::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::ToolResult(tool_result))
                    .build()
                    .expect("message build should not fail");
                bedrock_messages.push(bedrock_msg);
            }
            Message::Chat { content, .. } => {
                // Map custom roles to user by default.
                let bedrock_msg = bedrock_types::Message::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(content.clone()))
                    .build()
                    .expect("message build should not fail");
                bedrock_messages.push(bedrock_msg);
            }
            Message::Remove { .. } => { /* Skip remove messages */ }
        }
    }

    (system_blocks, bedrock_messages)
}

/// Parse a Bedrock response message into a Synaptic `Message`.
fn parse_bedrock_message(msg: &bedrock_types::Message) -> Message {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in msg.content() {
        match block {
            ContentBlock::Text(text) => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse(tool_use) => {
                tool_calls.push(ToolCall {
                    id: tool_use.tool_use_id().to_string(),
                    name: tool_use.name().to_string(),
                    arguments: document_to_json_value(tool_use.input()),
                });
            }
            _ => { /* Ignore other content block types for now */ }
        }
    }

    let content = text_parts.join("");

    if tool_calls.is_empty() {
        Message::ai(content)
    } else {
        Message::ai_with_tool_calls(content, tool_calls)
    }
}

// ---------------------------------------------------------------------------
// Document <-> serde_json::Value conversion
// ---------------------------------------------------------------------------

/// Convert a `serde_json::Value` to an `aws_smithy_types::Document`.
pub(crate) fn json_value_to_document(value: &Value) -> SmithyDocument {
    match value {
        Value::Null => SmithyDocument::Null,
        Value::Bool(b) => SmithyDocument::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SmithyDocument::Number(aws_smithy_types::Number::NegInt(i))
            } else if let Some(u) = n.as_u64() {
                SmithyDocument::Number(aws_smithy_types::Number::PosInt(u))
            } else if let Some(f) = n.as_f64() {
                SmithyDocument::Number(aws_smithy_types::Number::Float(f))
            } else {
                SmithyDocument::Null
            }
        }
        Value::String(s) => SmithyDocument::String(s.clone()),
        Value::Array(arr) => {
            SmithyDocument::Array(arr.iter().map(json_value_to_document).collect())
        }
        Value::Object(obj) => {
            let map: HashMap<String, SmithyDocument> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_document(v)))
                .collect();
            SmithyDocument::Object(map)
        }
    }
}

/// Convert an `aws_smithy_types::Document` to a `serde_json::Value`.
pub(crate) fn document_to_json_value(doc: &SmithyDocument) -> Value {
    match doc {
        SmithyDocument::Null => Value::Null,
        SmithyDocument::Bool(b) => Value::Bool(*b),
        SmithyDocument::Number(n) => match *n {
            aws_smithy_types::Number::PosInt(u) => {
                serde_json::json!(u)
            }
            aws_smithy_types::Number::NegInt(i) => {
                serde_json::json!(i)
            }
            aws_smithy_types::Number::Float(f) => {
                serde_json::json!(f)
            }
        },
        SmithyDocument::String(s) => Value::String(s.clone()),
        SmithyDocument::Array(arr) => {
            Value::Array(arr.iter().map(document_to_json_value).collect())
        }
        SmithyDocument::Object(obj) => {
            let map: serde_json::Map<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), document_to_json_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_value_to_document_round_trip() {
        let original = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name"]
        });

        let doc = json_value_to_document(&original);
        let back = document_to_json_value(&doc);
        assert_eq!(original, back);
    }

    #[test]
    fn json_value_to_document_primitives() {
        assert!(matches!(
            json_value_to_document(&Value::Null),
            SmithyDocument::Null
        ));
        assert!(matches!(
            json_value_to_document(&Value::Bool(true)),
            SmithyDocument::Bool(true)
        ));
        assert!(matches!(
            json_value_to_document(&serde_json::json!("hello")),
            SmithyDocument::String(_)
        ));
    }

    #[test]
    fn convert_system_messages() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::human("Hello!"),
        ];

        let (system_blocks, bedrock_messages) = convert_messages(&messages);
        assert_eq!(system_blocks.len(), 1);
        assert_eq!(bedrock_messages.len(), 1);
    }

    #[test]
    fn convert_tool_messages() {
        let messages = vec![
            Message::human("What is the weather?"),
            Message::ai_with_tool_calls(
                "",
                vec![ToolCall {
                    id: "tc_1".to_string(),
                    name: "get_weather".to_string(),
                    arguments: serde_json::json!({"city": "NYC"}),
                }],
            ),
            Message::tool("Sunny, 72F", "tc_1"),
        ];

        let (system_blocks, bedrock_messages) = convert_messages(&messages);
        assert!(system_blocks.is_empty());
        assert_eq!(bedrock_messages.len(), 3);

        // First message is user.
        assert_eq!(*bedrock_messages[0].role(), ConversationRole::User);
        // Second is assistant with tool use.
        assert_eq!(*bedrock_messages[1].role(), ConversationRole::Assistant);
        // Third is user with tool result.
        assert_eq!(*bedrock_messages[2].role(), ConversationRole::User);
    }

    #[test]
    fn convert_remove_messages_are_skipped() {
        let messages = vec![
            Message::human("Hi"),
            Message::remove("some-id"),
            Message::ai("Hello!"),
        ];

        let (_, bedrock_messages) = convert_messages(&messages);
        assert_eq!(bedrock_messages.len(), 2);
    }

    #[test]
    fn parse_text_only_message() {
        let msg = bedrock_types::Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text("Hello world".to_string()))
            .build()
            .unwrap();

        let parsed = parse_bedrock_message(&msg);
        assert!(parsed.is_ai());
        assert_eq!(parsed.content(), "Hello world");
        assert!(parsed.tool_calls().is_empty());
    }

    #[test]
    fn parse_message_with_tool_use() {
        let tool_use = ToolUseBlock::builder()
            .tool_use_id("tc_1")
            .name("calculator")
            .input(json_value_to_document(&serde_json::json!({"expr": "1+1"})))
            .build()
            .unwrap();

        let msg = bedrock_types::Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text("Let me calculate.".to_string()))
            .content(ContentBlock::ToolUse(tool_use))
            .build()
            .unwrap();

        let parsed = parse_bedrock_message(&msg);
        assert!(parsed.is_ai());
        assert_eq!(parsed.content(), "Let me calculate.");
        assert_eq!(parsed.tool_calls().len(), 1);
        assert_eq!(parsed.tool_calls()[0].id, "tc_1");
        assert_eq!(parsed.tool_calls()[0].name, "calculator");
        assert_eq!(parsed.tool_calls()[0].arguments, serde_json::json!({"expr": "1+1"}));
    }
}
