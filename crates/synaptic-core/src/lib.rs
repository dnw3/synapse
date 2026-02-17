use std::collections::HashMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// ---------------------------------------------------------------------------
// ContentBlock — multimodal message content
// ---------------------------------------------------------------------------

/// A block of content within a message, supporting multimodal inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Audio {
        url: String,
    },
    Video {
        url: String,
    },
    File {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    Data {
        data: Value,
    },
    Reasoning {
        content: String,
    },
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// Represents a chat message. Tagged enum with System, Human, AI, and Tool variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        additional_kwargs: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        response_metadata: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content_blocks: Vec<ContentBlock>,
    },
    #[serde(rename = "human")]
    Human {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        additional_kwargs: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        response_metadata: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content_blocks: Vec<ContentBlock>,
    },
    #[serde(rename = "assistant")]
    AI {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        additional_kwargs: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        response_metadata: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content_blocks: Vec<ContentBlock>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage_metadata: Option<TokenUsage>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        invalid_tool_calls: Vec<InvalidToolCall>,
    },
    #[serde(rename = "tool")]
    Tool {
        content: String,
        tool_call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        additional_kwargs: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        response_metadata: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content_blocks: Vec<ContentBlock>,
    },
    #[serde(rename = "chat")]
    Chat {
        custom_role: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        additional_kwargs: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        response_metadata: HashMap<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content_blocks: Vec<ContentBlock>,
    },
    /// A special message that signals removal of a message by its ID.
    /// Used in message history management.
    #[serde(rename = "remove")]
    Remove {
        /// ID of the message to remove.
        id: String,
    },
}

/// Helper macro to set a shared field across all Message variants.
/// Note: Remove variant has no common fields, so it is a no-op.
macro_rules! set_message_field {
    ($self:expr, $field:ident, $value:expr) => {
        match $self {
            Message::System { $field, .. } => *$field = $value,
            Message::Human { $field, .. } => *$field = $value,
            Message::AI { $field, .. } => *$field = $value,
            Message::Tool { $field, .. } => *$field = $value,
            Message::Chat { $field, .. } => *$field = $value,
            Message::Remove { .. } => { /* Remove has no common fields */ }
        }
    };
}

/// Helper macro to get a shared field from all Message variants.
/// Note: Remove variant panics — callers handle Remove before using this macro.
macro_rules! get_message_field {
    ($self:expr, $field:ident) => {
        match $self {
            Message::System { $field, .. } => $field,
            Message::Human { $field, .. } => $field,
            Message::AI { $field, .. } => $field,
            Message::Tool { $field, .. } => $field,
            Message::Chat { $field, .. } => $field,
            Message::Remove { .. } => unreachable!("get_message_field called on Remove variant"),
        }
    };
}

impl Message {
    // -- Factory methods -----------------------------------------------------

    pub fn system(content: impl Into<String>) -> Self {
        Message::System {
            content: content.into(),
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
        }
    }

    pub fn human(content: impl Into<String>) -> Self {
        Message::Human {
            content: content.into(),
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
        }
    }

    pub fn ai(content: impl Into<String>) -> Self {
        Message::AI {
            content: content.into(),
            tool_calls: vec![],
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
            usage_metadata: None,
            invalid_tool_calls: Vec::new(),
        }
    }

    pub fn ai_with_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Message::AI {
            content: content.into(),
            tool_calls,
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
            usage_metadata: None,
            invalid_tool_calls: Vec::new(),
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Message::Tool {
            content: content.into(),
            tool_call_id: tool_call_id.into(),
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
        }
    }

    pub fn chat(role: impl Into<String>, content: impl Into<String>) -> Self {
        Message::Chat {
            custom_role: role.into(),
            content: content.into(),
            id: None,
            name: None,
            additional_kwargs: HashMap::new(),
            response_metadata: HashMap::new(),
            content_blocks: Vec::new(),
        }
    }

    /// Create a Remove message that signals removal of a message by its ID.
    pub fn remove(id: impl Into<String>) -> Self {
        Message::Remove { id: id.into() }
    }

    // -- Builder methods -----------------------------------------------------

    pub fn with_id(mut self, value: impl Into<String>) -> Self {
        set_message_field!(&mut self, id, Some(value.into()));
        self
    }

    pub fn with_name(mut self, value: impl Into<String>) -> Self {
        set_message_field!(&mut self, name, Some(value.into()));
        self
    }

    pub fn with_additional_kwarg(mut self, key: impl Into<String>, value: Value) -> Self {
        match &mut self {
            Message::System {
                additional_kwargs, ..
            }
            | Message::Human {
                additional_kwargs, ..
            }
            | Message::AI {
                additional_kwargs, ..
            }
            | Message::Tool {
                additional_kwargs, ..
            }
            | Message::Chat {
                additional_kwargs, ..
            } => {
                additional_kwargs.insert(key.into(), value);
            }
            Message::Remove { .. } => { /* Remove has no additional_kwargs */ }
        }
        self
    }

    pub fn with_response_metadata_entry(mut self, key: impl Into<String>, value: Value) -> Self {
        match &mut self {
            Message::System {
                response_metadata, ..
            }
            | Message::Human {
                response_metadata, ..
            }
            | Message::AI {
                response_metadata, ..
            }
            | Message::Tool {
                response_metadata, ..
            }
            | Message::Chat {
                response_metadata, ..
            } => {
                response_metadata.insert(key.into(), value);
            }
            Message::Remove { .. } => { /* Remove has no response_metadata */ }
        }
        self
    }

    pub fn with_content_blocks(mut self, blocks: Vec<ContentBlock>) -> Self {
        set_message_field!(&mut self, content_blocks, blocks);
        self
    }

    pub fn with_usage_metadata(mut self, usage: TokenUsage) -> Self {
        if let Message::AI { usage_metadata, .. } = &mut self {
            *usage_metadata = Some(usage);
        }
        self
    }

    // -- Accessor methods ----------------------------------------------------

    pub fn content(&self) -> &str {
        match self {
            Message::Remove { .. } => "",
            other => get_message_field!(other, content),
        }
    }

    pub fn role(&self) -> &str {
        match self {
            Message::System { .. } => "system",
            Message::Human { .. } => "human",
            Message::AI { .. } => "assistant",
            Message::Tool { .. } => "tool",
            Message::Chat { custom_role, .. } => custom_role,
            Message::Remove { .. } => "remove",
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(self, Message::System { .. })
    }

    pub fn is_human(&self) -> bool {
        matches!(self, Message::Human { .. })
    }

    pub fn is_ai(&self) -> bool {
        matches!(self, Message::AI { .. })
    }

    pub fn is_tool(&self) -> bool {
        matches!(self, Message::Tool { .. })
    }

    pub fn is_chat(&self) -> bool {
        matches!(self, Message::Chat { .. })
    }

    pub fn is_remove(&self) -> bool {
        matches!(self, Message::Remove { .. })
    }

    pub fn tool_calls(&self) -> &[ToolCall] {
        match self {
            Message::AI { tool_calls, .. } => tool_calls,
            _ => &[],
        }
    }

    pub fn tool_call_id(&self) -> Option<&str> {
        match self {
            Message::Tool { tool_call_id, .. } => Some(tool_call_id),
            _ => None,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Message::Remove { id } => Some(id),
            other => get_message_field!(other, id).as_deref(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Message::Remove { .. } => None,
            other => get_message_field!(other, name).as_deref(),
        }
    }

    pub fn additional_kwargs(&self) -> &HashMap<String, Value> {
        match self {
            Message::System {
                additional_kwargs, ..
            }
            | Message::Human {
                additional_kwargs, ..
            }
            | Message::AI {
                additional_kwargs, ..
            }
            | Message::Tool {
                additional_kwargs, ..
            }
            | Message::Chat {
                additional_kwargs, ..
            } => additional_kwargs,
            Message::Remove { .. } => {
                static EMPTY: std::sync::OnceLock<HashMap<String, Value>> =
                    std::sync::OnceLock::new();
                EMPTY.get_or_init(HashMap::new)
            }
        }
    }

    pub fn response_metadata(&self) -> &HashMap<String, Value> {
        match self {
            Message::System {
                response_metadata, ..
            }
            | Message::Human {
                response_metadata, ..
            }
            | Message::AI {
                response_metadata, ..
            }
            | Message::Tool {
                response_metadata, ..
            }
            | Message::Chat {
                response_metadata, ..
            } => response_metadata,
            Message::Remove { .. } => {
                static EMPTY: std::sync::OnceLock<HashMap<String, Value>> =
                    std::sync::OnceLock::new();
                EMPTY.get_or_init(HashMap::new)
            }
        }
    }

    pub fn content_blocks(&self) -> &[ContentBlock] {
        match self {
            Message::Remove { .. } => &[],
            other => get_message_field!(other, content_blocks),
        }
    }

    /// Return the remove ID if this is a Remove message.
    pub fn remove_id(&self) -> Option<&str> {
        match self {
            Message::Remove { id } => Some(id),
            _ => None,
        }
    }

    pub fn usage_metadata(&self) -> Option<&TokenUsage> {
        match self {
            Message::AI { usage_metadata, .. } => usage_metadata.as_ref(),
            _ => None,
        }
    }

    pub fn invalid_tool_calls(&self) -> &[InvalidToolCall] {
        match self {
            Message::AI {
                invalid_tool_calls, ..
            } => invalid_tool_calls,
            _ => &[],
        }
    }
}

// ---------------------------------------------------------------------------
// Message utility functions
// ---------------------------------------------------------------------------

/// Filter messages by type, name, or id.
pub fn filter_messages(
    messages: &[Message],
    include_types: Option<&[&str]>,
    exclude_types: Option<&[&str]>,
    include_names: Option<&[&str]>,
    exclude_names: Option<&[&str]>,
    include_ids: Option<&[&str]>,
    exclude_ids: Option<&[&str]>,
) -> Vec<Message> {
    messages
        .iter()
        .filter(|msg| {
            if let Some(include) = include_types {
                if !include.contains(&msg.role()) {
                    return false;
                }
            }
            if let Some(exclude) = exclude_types {
                if exclude.contains(&msg.role()) {
                    return false;
                }
            }
            if let Some(include) = include_names {
                match msg.name() {
                    Some(name) => {
                        if !include.contains(&name) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            if let Some(exclude) = exclude_names {
                if let Some(name) = msg.name() {
                    if exclude.contains(&name) {
                        return false;
                    }
                }
            }
            if let Some(include) = include_ids {
                match msg.id() {
                    Some(id) => {
                        if !include.contains(&id) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            if let Some(exclude) = exclude_ids {
                if let Some(id) = msg.id() {
                    if exclude.contains(&id) {
                        return false;
                    }
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Strategy for trimming messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimStrategy {
    /// Keep the first messages that fit within the token budget.
    First,
    /// Keep the last messages that fit within the token budget.
    Last,
}

/// Trim messages to fit within a token budget.
///
/// `token_counter` receives a single message and returns its token count.
/// When `include_system` is true and `strategy` is `Last`, the leading system
/// message is always preserved.
pub fn trim_messages(
    messages: Vec<Message>,
    max_tokens: usize,
    token_counter: impl Fn(&Message) -> usize,
    strategy: TrimStrategy,
    include_system: bool,
) -> Vec<Message> {
    if messages.is_empty() {
        return messages;
    }

    match strategy {
        TrimStrategy::First => {
            let mut result = Vec::new();
            let mut total = 0;
            for msg in messages {
                let count = token_counter(&msg);
                if total + count > max_tokens {
                    break;
                }
                total += count;
                result.push(msg);
            }
            result
        }
        TrimStrategy::Last => {
            let (system_msg, rest) = if include_system && messages[0].is_system() {
                (Some(messages[0].clone()), &messages[1..])
            } else {
                (None, messages.as_slice())
            };

            let system_tokens = system_msg.as_ref().map(&token_counter).unwrap_or(0);
            let budget = max_tokens.saturating_sub(system_tokens);

            let mut selected = Vec::new();
            let mut total = 0;
            for msg in rest.iter().rev() {
                let count = token_counter(msg);
                if total + count > budget {
                    break;
                }
                total += count;
                selected.push(msg.clone());
            }
            selected.reverse();

            let mut result = Vec::new();
            if let Some(sys) = system_msg {
                result.push(sys);
            }
            result.extend(selected);
            result
        }
    }
}

/// Merge consecutive messages of the same role into a single message.
pub fn merge_message_runs(messages: Vec<Message>) -> Vec<Message> {
    if messages.is_empty() {
        return messages;
    }

    let mut result: Vec<Message> = Vec::new();

    for msg in messages {
        let should_merge = result
            .last()
            .map(|last| last.role() == msg.role())
            .unwrap_or(false);

        if should_merge {
            let last = result.last_mut().unwrap();
            // Merge content
            let merged_content = format!("{}\n{}", last.content(), msg.content());
            match last {
                Message::System { content, .. } => *content = merged_content,
                Message::Human { content, .. } => *content = merged_content,
                Message::AI {
                    content,
                    tool_calls,
                    invalid_tool_calls,
                    ..
                } => {
                    *content = merged_content;
                    tool_calls.extend(msg.tool_calls().to_vec());
                    invalid_tool_calls.extend(msg.invalid_tool_calls().to_vec());
                }
                Message::Tool { content, .. } => *content = merged_content,
                Message::Chat { content, .. } => *content = merged_content,
                Message::Remove { .. } => { /* Remove messages are not merged */ }
            }
        } else {
            result.push(msg);
        }
    }

    result
}

/// Convert messages to a human-readable buffer string.
pub fn get_buffer_string(messages: &[Message], human_prefix: &str, ai_prefix: &str) -> String {
    messages
        .iter()
        .map(|msg| {
            let prefix = match msg {
                Message::System { .. } => "System",
                Message::Human { .. } => human_prefix,
                Message::AI { .. } => ai_prefix,
                Message::Tool { .. } => "Tool",
                Message::Chat { custom_role, .. } => custom_role.as_str(),
                Message::Remove { .. } => "Remove",
            };
            format!("{prefix}: {}", msg.content())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// AIMessageChunk
// ---------------------------------------------------------------------------

/// A streaming chunk from an AI model response. Supports merge via `+`/`+=` operators and conversion to `Message` via `into_message()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AIMessageChunk {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_call_chunks: Vec<ToolCallChunk>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_tool_calls: Vec<InvalidToolCall>,
}

impl AIMessageChunk {
    pub fn into_message(self) -> Message {
        Message::ai_with_tool_calls(self.content, self.tool_calls)
    }
}

impl std::ops::Add for AIMessageChunk {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl std::ops::AddAssign for AIMessageChunk {
    fn add_assign(&mut self, rhs: Self) {
        self.content.push_str(&rhs.content);
        self.tool_calls.extend(rhs.tool_calls);
        self.tool_call_chunks.extend(rhs.tool_call_chunks);
        self.invalid_tool_calls.extend(rhs.invalid_tool_calls);
        if self.id.is_none() {
            self.id = rhs.id;
        }
        match (&mut self.usage, rhs.usage) {
            (Some(u), Some(rhs_u)) => {
                u.input_tokens += rhs_u.input_tokens;
                u.output_tokens += rhs_u.output_tokens;
                u.total_tokens += rhs_u.total_tokens;
            }
            (None, Some(rhs_u)) => {
                self.usage = Some(rhs_u);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tool-related types
// ---------------------------------------------------------------------------

/// Represents a tool invocation requested by an AI model, with an ID, function name, and JSON arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// A tool call that failed to parse correctly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidToolCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    pub error: String,
}

/// A partial tool call chunk received during streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallChunk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

/// Schema definition for a tool, including its name, description, and JSON Schema for parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Controls how the model selects tools: Auto, Required, None, or a Specific named tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    Auto,
    Required,
    None,
    Specific(String),
}

// ---------------------------------------------------------------------------
// Chat request / response
// ---------------------------------------------------------------------------

/// A request to a chat model containing messages, optional tool definitions, and tool choice configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl ChatRequest {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            tools: vec![],
            tool_choice: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }
}

/// A response from a chat model containing the AI message and optional token usage statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: Message,
    pub usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// Token usage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_details: Option<InputTokenDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_details: Option<OutputTokenDetails>,
}

/// Detailed breakdown of input token usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InputTokenDetails {
    #[serde(default)]
    pub cached: u32,
    #[serde(default)]
    pub audio: u32,
}

/// Detailed breakdown of output token usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutputTokenDetails {
    #[serde(default)]
    pub reasoning: u32,
    #[serde(default)]
    pub audio: u32,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Lifecycle events emitted during agent execution, used by `CallbackHandler` implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    RunStarted {
        run_id: String,
        session_id: String,
    },
    RunStep {
        run_id: String,
        step: usize,
    },
    LlmCalled {
        run_id: String,
        message_count: usize,
    },
    ToolCalled {
        run_id: String,
        tool_name: String,
    },
    RunFinished {
        run_id: String,
        output: String,
    },
    RunFailed {
        run_id: String,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Unified error type for the Synapse framework with variants covering all subsystems.
#[derive(Debug, Error)]
pub enum SynapseError {
    #[error("prompt error: {0}")]
    Prompt(String),
    #[error("model error: {0}")]
    Model(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("memory error: {0}")]
    Memory(String),
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("parsing error: {0}")]
    Parsing(String),
    #[error("callback error: {0}")]
    Callback(String),
    #[error("max steps exceeded: {max_steps}")]
    MaxStepsExceeded { max_steps: usize },
    #[error("embedding error: {0}")]
    Embedding(String),
    #[error("vector store error: {0}")]
    VectorStore(String),
    #[error("retriever error: {0}")]
    Retriever(String),
    #[error("loader error: {0}")]
    Loader(String),
    #[error("splitter error: {0}")]
    Splitter(String),
    #[error("graph error: {0}")]
    Graph(String),
    #[error("cache error: {0}")]
    Cache(String),
    #[error("config error: {0}")]
    Config(String),
}

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

/// Type alias for a pinned, boxed async stream of `AIMessageChunk` results.
pub type ChatStream<'a> =
    Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapseError>> + Send + 'a>>;

/// The core trait for language model providers. Implementations provide `chat()` for single responses and optionally `stream_chat()` for streaming.
#[async_trait]
pub trait ChatModel: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError>;

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        Box::pin(async_stream::stream! {
            match self.chat(request).await {
                Ok(response) => {
                    yield Ok(AIMessageChunk {
                        content: response.message.content().to_string(),
                        tool_calls: response.message.tool_calls().to_vec(),
                        usage: response.usage,
                        ..Default::default()
                    });
                }
                Err(e) => yield Err(e),
            }
        })
    }
}

/// Defines an executable tool that can be called by an AI model. Each tool has a name, description, JSON schema for parameters, and an async `call()` method.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn call(&self, args: Value) -> Result<Value, SynapseError>;
}

/// Persistent storage for conversation message history, keyed by session ID.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError>;
    async fn clear(&self, session_id: &str) -> Result<(), SynapseError>;
}

/// Handler for lifecycle events during agent execution. Receives `RunEvent` notifications at each stage.
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapseError>;
}

// ---------------------------------------------------------------------------
// RunnableConfig
// ---------------------------------------------------------------------------

/// Runtime configuration passed through runnable chains, including tags, metadata, concurrency limits, and run identification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnableConfig {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
    #[serde(default)]
    pub recursion_limit: Option<usize>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub run_name: Option<String>,
}

impl RunnableConfig {
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_run_name(mut self, name: impl Into<String>) -> Self {
        self.run_name = Some(name.into());
        self
    }

    pub fn with_run_id(mut self, id: impl Into<String>) -> Self {
        self.run_id = Some(id.into());
        self
    }

    pub fn with_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = Some(max);
        self
    }

    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = Some(limit);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}
