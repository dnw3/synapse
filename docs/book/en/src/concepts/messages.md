# Messages

Messages are the fundamental unit of communication in Synaptic. Every interaction with an LLM -- whether a simple question, a multi-turn conversation, a tool call, or a streaming response -- is expressed as a sequence of messages. This page explains the message system's design, its variants, and the utilities that operate on message sequences.

## Message as a Tagged Enum

`Message` is a Rust enum with six variants, serialized with `#[serde(tag = "role")]`:

| Variant | Role String | Purpose |
|---------|-------------|---------|
| `System` | `"system"` | Instructions to the model about behavior and constraints |
| `Human` | `"human"` | User input |
| `AI` | `"assistant"` | Model responses, optionally carrying tool calls |
| `Tool` | `"tool"` | Results from tool execution, linked by `tool_call_id` |
| `Chat` | custom | Messages with a user-defined role for special protocols |
| `Remove` | `"remove"` | A signal to remove a message by ID from history |

This is a tagged enum, not a trait hierarchy. Pattern matching is exhaustive, serialization is automatic, and the compiler enforces that every code path handles every variant.

### Why an Enum?

An enum makes it impossible to construct an invalid message. An AI message always has a `tool_calls` field (even if empty). A Tool message always has a `tool_call_id`. A System message never has tool calls. These invariants are enforced by the type system rather than by runtime checks.

## Creating Messages

Synaptic provides factory methods rather than exposing struct literals. This keeps the API stable even as internal fields are added:

```rust
use synaptic::core::Message;

// Basic messages
let sys = Message::system("You are a helpful assistant.");
let user = Message::human("What is the weather?");
let reply = Message::ai("The weather is sunny today.");

// AI message with tool calls
let with_tools = Message::ai_with_tool_calls("Let me check.", vec![tool_call]);

// Tool result linked to a specific call
let result = Message::tool("72 degrees", "call_abc123");

// Custom role
let custom = Message::chat("moderator", "This message is approved.");

// Removal signal
let remove = Message::remove("msg_id_to_remove");
```

### Builder Methods

Factory methods create messages with default (empty) optional fields. Builder methods let you set them:

```rust
let msg = Message::human("Hello")
    .with_id("msg_001")
    .with_name("Alice")
    .with_content_blocks(vec![
        ContentBlock::Text { text: "Hello".into() },
        ContentBlock::Image { url: "https://example.com/photo.jpg".into(), detail: None },
    ]);
```

Available builders: `with_id()`, `with_name()`, `with_additional_kwarg()`, `with_response_metadata_entry()`, `with_content_blocks()`, `with_usage_metadata()` (AI only).

## Accessing Message Fields

Accessor methods work uniformly across variants:

```rust
let msg = Message::ai("Hello world");

msg.content()       // "Hello world"
msg.role()          // "assistant"
msg.is_ai()         // true
msg.is_human()      // false
msg.tool_calls()    // &[] (empty slice for non-AI messages)
msg.tool_call_id()  // None (only Some for Tool messages)
msg.id()            // None (unless set with .with_id())
msg.name()          // None (unless set with .with_name())
```

Type-check methods: `is_system()`, `is_human()`, `is_ai()`, `is_tool()`, `is_chat()`, `is_remove()`.

The `Remove` variant is special: it carries only an `id` field. Calling `content()` on it returns `""`, and `name()` returns `None`. The `remove_id()` method returns `Some(&str)` only for Remove messages.

## Common Fields

Every message variant (except `Remove`) carries these fields:

- **`content: String`** -- the text content
- **`id: Option<String>`** -- optional unique identifier
- **`name: Option<String>`** -- optional sender name
- **`additional_kwargs: HashMap<String, Value>`** -- extensible key-value metadata
- **`response_metadata: HashMap<String, Value>`** -- provider-specific response metadata
- **`content_blocks: Vec<ContentBlock>`** -- multimodal content (text, images, audio, video, files, data, reasoning)

The AI variant additionally carries:
- **`tool_calls: Vec<ToolCall>`** -- structured tool invocations
- **`invalid_tool_calls: Vec<InvalidToolCall>`** -- tool calls that failed to parse
- **`usage_metadata: Option<TokenUsage>`** -- token usage from the provider

The Tool variant additionally carries:
- **`tool_call_id: String`** -- links back to the ToolCall that produced this result

## Streaming with AIMessageChunk

When streaming responses from an LLM, content arrives in chunks. The `AIMessageChunk` struct represents a single chunk:

```rust
pub struct AIMessageChunk {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
    pub id: Option<String>,
    pub tool_call_chunks: Vec<ToolCallChunk>,
    pub invalid_tool_calls: Vec<InvalidToolCall>,
}
```

Chunks support the `+` and `+=` operators to merge them incrementally:

```rust
let mut accumulated = AIMessageChunk::default();
accumulated += chunk1;  // content is concatenated
accumulated += chunk2;  // tool_calls are extended
accumulated += chunk3;  // usage is summed

// Convert the accumulated chunk to a Message
let message = accumulated.into_message();
```

The merge semantics are:
- `content` is concatenated via `push_str`
- `tool_calls`, `tool_call_chunks`, and `invalid_tool_calls` are extended
- `id` takes the first non-None value
- `usage` is summed field-by-field (input_tokens, output_tokens, total_tokens)

## Multimodal Content

The `ContentBlock` enum supports rich content types beyond plain text:

| Variant | Fields | Purpose |
|---------|--------|---------|
| `Text` | `text` | Plain text |
| `Image` | `url`, `detail` | Image reference with optional detail level |
| `Audio` | `url` | Audio reference |
| `Video` | `url` | Video reference |
| `File` | `url`, `mime_type` | Generic file reference |
| `Data` | `data: Value` | Arbitrary structured data |
| `Reasoning` | `content` | Model reasoning/chain-of-thought |

Content blocks are carried alongside the `content` string field, allowing messages to contain both a text summary and structured multimodal data.

## Message Utility Functions

Synaptic provides four utility functions for working with message sequences:

### filter_messages

Filter messages by role, name, or ID with include/exclude lists:

```rust
use synaptic::core::filter_messages;

let humans_only = filter_messages(
    &messages,
    Some(&["human"]),  // include_types
    None,              // exclude_types
    None, None,        // include/exclude names
    None, None,        // include/exclude ids
);
```

### trim_messages

Trim a message sequence to fit within a token budget:

```rust
use synaptic::core::{trim_messages, TrimStrategy};

let trimmed = trim_messages(
    messages,
    4096,                       // max tokens
    |msg| msg.content().len() / 4,  // token counter function
    TrimStrategy::Last,         // keep most recent
    true,                       // always preserve system message
);
```

`TrimStrategy::First` keeps messages from the beginning. `TrimStrategy::Last` keeps messages from the end, optionally preserving the leading system message.

### merge_message_runs

Merge consecutive messages of the same role into a single message:

```rust
use synaptic::core::merge_message_runs;

let merged = merge_message_runs(vec![
    Message::human("Hello"),
    Message::human("How are you?"),
    Message::ai("I'm fine"),
]);
// Result: [Human("Hello\nHow are you?"), AI("I'm fine")]
```

For AI messages, tool calls and invalid tool calls are also merged.

### get_buffer_string

Convert a message sequence to a human-readable string:

```rust
use synaptic::core::get_buffer_string;

let text = get_buffer_string(&messages, "Human", "AI");
// "System: You are helpful.\nHuman: Hello\nAI: Hi there!"
```

## Serialization

Messages serialize as JSON with a `role` discriminator field:

```json
{
  "role": "assistant",
  "content": "Hello!",
  "tool_calls": [],
  "id": null,
  "name": null
}
```

The AI variant serializes its role as `"assistant"` (matching OpenAI convention), while `role()` returns `"assistant"` at runtime as well. Empty collections and None optionals are omitted from serialization via `skip_serializing_if` attributes.

This serialization format is compatible with LangChain's message schema, making it straightforward to exchange message histories between Synaptic and Python-based systems.

## See Also

- [Message Types](../how-to/messages/types.md) -- detailed examples for each message variant
- [Filter & Trim](../how-to/messages/filter-trim.md) -- filtering and trimming message sequences
- [Merge Runs](../how-to/messages/merge-runs.md) -- merging consecutive same-role messages
- [Memory](memory.md) -- how messages are stored and managed across sessions
