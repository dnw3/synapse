# Phase 1: Core Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix compilation errors, refactor Message to enum variants, add AIMessageChunk/RunnableConfig, expand SynapseError, and achieve a fully green `cargo test --workspace`.

**Architecture:** Replace flat `Message { role, content }` struct with a tagged enum (`System`, `Human`, `AI`, `Tool`) carrying variant-specific fields (tool_calls, tool_call_id). Simplify `ChatResponse` by moving tool_calls into the AI message variant. Add streaming chunk type and runtime config struct.

**Tech Stack:** Rust 2021, async-trait, serde/serde_json, thiserror, tokio

---

### Task 1: Fix Immediate Compilation Errors

Get `cargo test --workspace` to a green baseline before refactoring.

**Files:**
- Modify: `crates/synapse-agents/tests/react_agent.rs`
- Modify: `examples/react_basic/src/main.rs`
- Delete: `crates/synapse-models/tests/provider_adapters.rs`
- Delete: `crates/synapse-models/tests/reliability.rs`

**Step 1: Add missing `usage: None` to react_agent test**

In `crates/synapse-agents/tests/react_agent.rs`, the `ScriptedModel` returns `ChatResponse` without the `usage` field. Add `usage: None` to both return sites:

```rust
// First branch (~line 18):
Ok(ChatResponse {
    message: Message::new(Role::Assistant, "calling tool"),
    tool_calls: vec![ToolCall {
        id: "call-1".to_string(),
        name: "add".to_string(),
        arguments: json!({"a": 1, "b": 2}),
    }],
    usage: None,
})

// Second branch (~line 28):
Ok(ChatResponse {
    message: Message::new(Role::Assistant, "result is 3"),
    tool_calls: vec![],
    usage: None,
})
```

**Step 2: Add missing `usage: None` to react_basic example**

In `examples/react_basic/src/main.rs`, same fix in `DemoModel::chat`:

```rust
// First branch (~line 20):
Ok(ChatResponse {
    message: Message::new(Role::Assistant, "I will use a tool to calculate this."),
    tool_calls: vec![ToolCall {
        id: "call-1".to_string(),
        name: "add".to_string(),
        arguments: json!({ "a": 7, "b": 5 }),
    }],
    usage: None,
})

// Second branch (~line 30):
Ok(ChatResponse {
    message: Message::new(Role::Assistant, "The result is 12."),
    tool_calls: vec![],
    usage: None,
})
```

**Step 3: Delete Phase 2 placeholder test files**

```bash
rm crates/synapse-models/tests/provider_adapters.rs
rm crates/synapse-models/tests/reliability.rs
```

These reference types (`OpenAiChatModel`, `AnthropicChatModel`, `GeminiChatModel`, `ProviderBackend`, `RetryChatModel`, `RateLimitedChatModel`, `RetryPolicy`) that don't exist yet.

**Step 4: Verify green build**

```bash
cargo test --workspace
```

Expected: All tests pass. No compilation errors.

**Step 5: Commit**

```bash
git add -A && git commit -m "fix: resolve compilation errors and remove Phase 2 placeholder tests"
```

---

### Task 2: Refactor Message to Enum

Replace `Role` enum + `Message` struct with a tagged `Message` enum where each variant carries its own fields.

**Files:**
- Modify: `crates/synapse-core/src/lib.rs`
- Test: `crates/synapse-core/tests/message.rs` (create)
- Modify: `crates/synapse-core/Cargo.toml`

**Step 1: Write the failing test**

Create `crates/synapse-core/tests/message.rs`:

```rust
use serde_json::json;
use synapse_core::{Message, ToolCall};

#[test]
fn system_message_factory() {
    let msg = Message::system("You are helpful");
    assert_eq!(msg.content(), "You are helpful");
    assert_eq!(msg.role(), "system");
    assert!(msg.is_system());
    assert!(!msg.is_human());
}

#[test]
fn human_message_factory() {
    let msg = Message::human("Hello");
    assert_eq!(msg.content(), "Hello");
    assert_eq!(msg.role(), "human");
    assert!(msg.is_human());
}

#[test]
fn ai_message_factory() {
    let msg = Message::ai("I can help");
    assert_eq!(msg.content(), "I can help");
    assert_eq!(msg.role(), "assistant");
    assert!(msg.is_ai());
    assert!(msg.tool_calls().is_empty());
}

#[test]
fn ai_message_with_tool_calls() {
    let msg = Message::ai_with_tool_calls(
        "calling tool",
        vec![ToolCall {
            id: "call-1".into(),
            name: "search".into(),
            arguments: json!({"q": "rust"}),
        }],
    );
    assert_eq!(msg.tool_calls().len(), 1);
    assert_eq!(msg.tool_calls()[0].name, "search");
}

#[test]
fn tool_message_factory() {
    let msg = Message::tool("result data", "call-1");
    assert_eq!(msg.content(), "result data");
    assert_eq!(msg.role(), "tool");
    assert!(msg.is_tool());
    assert_eq!(msg.tool_call_id(), Some("call-1"));
}

#[test]
fn tool_call_id_none_for_non_tool() {
    let msg = Message::human("hi");
    assert_eq!(msg.tool_call_id(), None);
}

#[test]
fn message_serde_roundtrip() {
    let msg = Message::ai_with_tool_calls(
        "using tool",
        vec![ToolCall {
            id: "c1".into(),
            name: "calc".into(),
            arguments: json!({"x": 1}),
        }],
    );
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
}

#[test]
fn message_serde_system_format() {
    let msg = Message::system("be helpful");
    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["role"], "system");
    assert_eq!(json["content"], "be helpful");
}

#[test]
fn message_serde_tool_calls_omitted_when_empty() {
    let msg = Message::ai("hello");
    let json = serde_json::to_value(&msg).unwrap();
    assert!(json.get("tool_calls").is_none());
}
```

**Step 2: Add tokio + serde_json dev-dependencies to synapse-core**

In `crates/synapse-core/Cargo.toml`, add:

```toml
[dev-dependencies]
serde_json.workspace = true
```

**Step 3: Run test to verify it fails**

```bash
cargo test -p synapse-core
```

Expected: FAIL — `Message::system`, `Message::human`, etc. don't exist yet.

**Step 4: Implement the new Message enum**

Replace the `Role` enum and `Message` struct in `crates/synapse-core/src/lib.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "human")]
    Human { content: String },
    #[serde(rename = "assistant")]
    AI {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    #[serde(rename = "tool")]
    Tool {
        content: String,
        tool_call_id: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message::System {
            content: content.into(),
        }
    }

    pub fn human(content: impl Into<String>) -> Self {
        Message::Human {
            content: content.into(),
        }
    }

    pub fn ai(content: impl Into<String>) -> Self {
        Message::AI {
            content: content.into(),
            tool_calls: vec![],
        }
    }

    pub fn ai_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Message::AI {
            content: content.into(),
            tool_calls,
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Message::Tool {
            content: content.into(),
            tool_call_id: tool_call_id.into(),
        }
    }

    pub fn content(&self) -> &str {
        match self {
            Message::System { content } => content,
            Message::Human { content } => content,
            Message::AI { content, .. } => content,
            Message::Tool { content, .. } => content,
        }
    }

    pub fn role(&self) -> &str {
        match self {
            Message::System { .. } => "system",
            Message::Human { .. } => "human",
            Message::AI { .. } => "assistant",
            Message::Tool { .. } => "tool",
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
}
```

Also add `Eq` to `ToolCall` (serde_json::Value implements Eq):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
```

Remove the old `Role` enum and `Message` struct (including the `Message::new` and `impl Message` block).

**Step 5: Simplify ChatResponse**

Remove `tool_calls` from `ChatResponse` — tool_calls now live on the AI message variant:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: Message,
    pub usage: Option<TokenUsage>,
}
```

**Step 6: Run the synapse-core tests**

```bash
cargo test -p synapse-core
```

Expected: All message tests PASS. Other crates will fail to compile (they still reference old API).

**Step 7: Commit**

```bash
git add -A && git commit -m "refactor: replace Message struct with tagged enum variants"
```

---

### Task 3: Update synapse-agents for New Message API

**Files:**
- Modify: `crates/synapse-agents/src/lib.rs`
- Modify: `crates/synapse-agents/tests/react_agent.rs`

**Step 1: Update ReActAgentExecutor**

In `crates/synapse-agents/src/lib.rs`:

Remove `Role` from imports:

```rust
use synapse_core::{
    Agent, CallbackHandler, ChatModel, ChatRequest, MemoryStore, Message, RunEvent,
    SynapseError,
};
```

Update the `run` method body (all Message construction and access):

```rust
// Line ~69: User message
self.memory
    .append(session_id, Message::human(input))
    .await?;

// Line ~81: System message
messages.push(Message::system(self.config.system_prompt.clone()));

// Line ~99-100: Check tool_calls and get content
if response.message.tool_calls().is_empty() {
    let output = response.message.content().to_string();

// Line ~110: Iterate tool calls (borrowed)
for call in response.message.tool_calls() {

// Line ~118: Clone arguments (borrowed reference)
let result = self.tools.execute(&call.name, call.arguments.clone()).await?;

// Line ~120: Tool message with call_id
self.memory
    .append(
        session_id,
        Message::tool(result.to_string(), &call.id),
    )
    .await?;
```

**Step 2: Update react_agent test**

In `crates/synapse-agents/tests/react_agent.rs`:

Update imports — remove `Role`, remove `ChatResponse` if unused (actually still needed for constructing responses):

```rust
use synapse_core::{
    Agent, ChatModel, ChatRequest, ChatResponse, Message, SynapseError, Tool,
    ToolCall,
};
```

Update `ScriptedModel::chat`:

```rust
async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
    let has_tool_result = request.messages.iter().any(|m| m.is_tool());

    if !has_tool_result {
        Ok(ChatResponse {
            message: Message::ai_with_tool_calls(
                "calling tool",
                vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "add".to_string(),
                    arguments: json!({"a": 1, "b": 2}),
                }],
            ),
            usage: None,
        })
    } else {
        Ok(ChatResponse {
            message: Message::ai("result is 3"),
            usage: None,
        })
    }
}
```

Update assertion in the test:

```rust
assert!(messages.iter().any(|m| m.is_tool()));
```

**Step 3: Run tests**

```bash
cargo test -p synapse-agents
```

Expected: PASS

**Step 4: Commit**

```bash
git add -A && git commit -m "refactor: update synapse-agents for new Message enum"
```

---

### Task 4: Update synapse-models

**Files:**
- Modify: `crates/synapse-models/src/lib.rs`

**Step 1: Update ScriptedChatModel**

The `ScriptedChatModel` stores `VecDeque<ChatResponse>`. `ChatResponse` no longer has `tool_calls`, so callers who construct `ChatResponse` to feed into `ScriptedChatModel` will use the new API. No changes needed to the `ScriptedChatModel` impl itself — it just pops from the queue.

Verify imports are correct (remove `Role` if imported — check current imports):

```rust
use synapse_core::{ChatModel, ChatRequest, ChatResponse, SynapseError};
```

These are unchanged. `ChatResponse` is still the same type name. No code changes needed in `lib.rs`.

**Step 2: Verify**

```bash
cargo test -p synapse-models
```

Expected: PASS (no test files remain after Task 1 deleted the placeholder tests)

**Step 3: Commit (skip if no changes)**

---

### Task 5: Update synapse-memory, synapse-callbacks, and Remaining Tests

**Files:**
- Modify: `crates/synapse-memory/tests/in_memory.rs`
- Check: `crates/synapse-callbacks/tests/recording.rs` (likely no changes — uses RunEvent not Message)

**Step 1: Update in_memory test**

In `crates/synapse-memory/tests/in_memory.rs`, replace `Role` usage:

```rust
use synapse_core::{MemoryStore, Message};
use synapse_memory::InMemoryStore;

#[tokio::test]
async fn stores_and_reads_messages_by_session() {
    let store = InMemoryStore::new();
    let msg = Message::human("hello");

    store
        .append("session-a", msg.clone())
        .await
        .expect("append should work");

    let loaded = store.load("session-a").await.expect("load should work");

    assert_eq!(loaded, vec![msg]);
}

#[tokio::test]
async fn isolates_sessions() {
    let store = InMemoryStore::new();
    store
        .append("session-a", Message::human("A"))
        .await
        .expect("append A");
    store
        .append("session-b", Message::human("B"))
        .await
        .expect("append B");

    let a = store.load("session-a").await.expect("load a");
    let b = store.load("session-b").await.expect("load b");

    assert_eq!(a[0].content(), "A");
    assert_eq!(b[0].content(), "B");
}
```

Key changes:
- `Message::new(Role::User, "hello")` → `Message::human("hello")`
- `a[0].content` → `a[0].content()`
- Remove `Role` from imports

**Step 2: Verify callbacks test needs no changes**

`crates/synapse-callbacks/tests/recording.rs` only uses `RunEvent`, not `Message`. No changes needed.

**Step 3: Run tests**

```bash
cargo test -p synapse-memory && cargo test -p synapse-callbacks
```

Expected: PASS

**Step 4: Commit**

```bash
git add -A && git commit -m "refactor: update memory tests for new Message enum"
```

---

### Task 6: Update Examples

**Files:**
- Modify: `examples/react_basic/src/main.rs`
- Modify: `examples/memory_chat/src/main.rs`
- Check: `examples/tool_calling_basic/src/main.rs` (no Message usage — skip)

**Step 1: Update react_basic example**

In `examples/react_basic/src/main.rs`:

Update imports:

```rust
use synapse_core::{
    Agent, ChatModel, ChatRequest, ChatResponse, Message, SynapseError, Tool, ToolCall,
};
```

Remove `Role` from imports.

Update `DemoModel::chat`:

```rust
async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
    let has_tool_output = request.messages.iter().any(|m| m.is_tool());
    if !has_tool_output {
        Ok(ChatResponse {
            message: Message::ai_with_tool_calls(
                "I will use a tool to calculate this.",
                vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "add".to_string(),
                    arguments: json!({ "a": 7, "b": 5 }),
                }],
            ),
            usage: None,
        })
    } else {
        Ok(ChatResponse {
            message: Message::ai("The result is 12."),
            usage: None,
        })
    }
}
```

**Step 2: Update memory_chat example**

In `examples/memory_chat/src/main.rs`:

```rust
use synapse_core::{MemoryStore, Message, SynapseError};
use synapse_memory::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), SynapseError> {
    let memory = InMemoryStore::new();
    let session_id = "demo-session";

    memory
        .append(session_id, Message::human("Hello, Synapse"))
        .await?;
    memory
        .append(
            session_id,
            Message::ai("Hello, how can I help you?"),
        )
        .await?;

    let transcript = memory.load(session_id).await?;
    for message in transcript {
        println!("{}: {}", message.role(), message.content());
    }

    Ok(())
}
```

Key changes:
- `Message::new(Role::User, ...)` → `Message::human(...)`
- `Message::new(Role::Assistant, ...)` → `Message::ai(...)`
- `message.role` → `message.role()` (method, returns &str)
- `message.content` → `message.content()` (method)
- Remove `Role` from imports

**Step 3: Verify all examples compile**

```bash
cargo build -p react_basic && cargo build -p memory_chat && cargo build -p tool_calling_basic
```

Expected: All compile successfully.

**Step 4: Commit**

```bash
git add -A && git commit -m "refactor: update examples for new Message enum"
```

---

### Task 7: Add AIMessageChunk for Streaming

**Files:**
- Modify: `crates/synapse-core/src/lib.rs`
- Modify: `crates/synapse-core/tests/message.rs`

**Step 1: Write the failing test**

Append to `crates/synapse-core/tests/message.rs`:

```rust
use synapse_core::{AIMessageChunk, TokenUsage};

#[test]
fn chunk_add_concatenates_content() {
    let a = AIMessageChunk {
        content: "Hello".into(),
        tool_calls: vec![],
        usage: None,
    };
    let b = AIMessageChunk {
        content: " world".into(),
        tool_calls: vec![],
        usage: None,
    };
    let merged = a + b;
    assert_eq!(merged.content, "Hello world");
}

#[test]
fn chunk_add_merges_tool_calls() {
    let a = AIMessageChunk {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "c1".into(),
            name: "search".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let b = AIMessageChunk {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "c2".into(),
            name: "calc".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let merged = a + b;
    assert_eq!(merged.tool_calls.len(), 2);
}

#[test]
fn chunk_add_merges_usage() {
    let a = AIMessageChunk {
        content: "a".into(),
        tool_calls: vec![],
        usage: Some(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
        }),
    };
    let b = AIMessageChunk {
        content: "b".into(),
        tool_calls: vec![],
        usage: Some(TokenUsage {
            input_tokens: 0,
            output_tokens: 3,
            total_tokens: 3,
        }),
    };
    let merged = a + b;
    let usage = merged.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 8);
    assert_eq!(usage.total_tokens, 18);
}

#[test]
fn chunk_add_assign_works() {
    let mut chunk = AIMessageChunk {
        content: "Hello".into(),
        tool_calls: vec![],
        usage: None,
    };
    chunk += AIMessageChunk {
        content: " world".into(),
        tool_calls: vec![],
        usage: None,
    };
    assert_eq!(chunk.content, "Hello world");
}

#[test]
fn chunk_into_message() {
    let chunk = AIMessageChunk {
        content: "final answer".into(),
        tool_calls: vec![ToolCall {
            id: "c1".into(),
            name: "tool".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let msg = chunk.into_message();
    assert!(msg.is_ai());
    assert_eq!(msg.content(), "final answer");
    assert_eq!(msg.tool_calls().len(), 1);
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test -p synapse-core
```

Expected: FAIL — `AIMessageChunk` doesn't exist.

**Step 3: Implement AIMessageChunk**

Add to `crates/synapse-core/src/lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AIMessageChunk {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
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
```

**Step 4: Run tests**

```bash
cargo test -p synapse-core
```

Expected: All PASS.

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add AIMessageChunk with streaming merge support"
```

---

### Task 8: Add RunnableConfig and Expand SynapseError

**Files:**
- Modify: `crates/synapse-core/src/lib.rs`
- Modify: `crates/synapse-core/tests/message.rs` (add config + error tests)

**Step 1: Write the failing tests**

Append to `crates/synapse-core/tests/message.rs` (or create `crates/synapse-core/tests/config.rs`):

Create `crates/synapse-core/tests/config.rs`:

```rust
use synapse_core::RunnableConfig;

#[test]
fn default_config_has_empty_fields() {
    let config = RunnableConfig::default();
    assert!(config.tags.is_empty());
    assert!(config.metadata.is_empty());
    assert!(config.max_concurrency.is_none());
    assert!(config.recursion_limit.is_none());
    assert!(config.run_id.is_none());
    assert!(config.run_name.is_none());
}

#[test]
fn config_builder_pattern() {
    let config = RunnableConfig::default()
        .with_tags(vec!["test".into()])
        .with_run_name("my-run");
    assert_eq!(config.tags, vec!["test"]);
    assert_eq!(config.run_name.as_deref(), Some("my-run"));
}
```

Create `crates/synapse-core/tests/error.rs`:

```rust
use synapse_core::SynapseError;

#[test]
fn error_variants_exist() {
    let errors = vec![
        SynapseError::Embedding("test".into()),
        SynapseError::VectorStore("test".into()),
        SynapseError::Retriever("test".into()),
        SynapseError::Loader("test".into()),
        SynapseError::Splitter("test".into()),
        SynapseError::Graph("test".into()),
        SynapseError::Cache("test".into()),
        SynapseError::Config("test".into()),
    ];
    for err in &errors {
        assert!(!err.to_string().is_empty());
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p synapse-core
```

Expected: FAIL — `RunnableConfig` and new error variants don't exist.

**Step 3: Add RunnableConfig**

Add to `crates/synapse-core/src/lib.rs`:

```rust
use std::collections::HashMap;

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
```

**Step 4: Expand SynapseError**

Add new variants to the `SynapseError` enum in `crates/synapse-core/src/lib.rs`:

```rust
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
```

**Step 5: Run tests**

```bash
cargo test -p synapse-core
```

Expected: All PASS.

**Step 6: Commit**

```bash
git add -A && git commit -m "feat: add RunnableConfig and expand SynapseError variants"
```

---

### Task 9: Add Metadata to Document

**Files:**
- Modify: `crates/synapse-retrieval/src/lib.rs`
- Modify: `crates/synapse-retrieval/Cargo.toml`
- Check: `crates/synapse-retrieval/tests/in_memory.rs` (should still pass — field access only)
- Check: `crates/synapse-loaders/src/lib.rs` (Document::new still works)
- Check: `crates/synapse-loaders/tests/text_loader.rs` (field access only)

**Step 1: Add serde_json dependency**

In `crates/synapse-retrieval/Cargo.toml`, add:

```toml
serde_json.workspace = true
```

**Step 2: Update Document struct**

In `crates/synapse-retrieval/src/lib.rs`, add metadata field:

```rust
use std::collections::{HashMap, HashSet};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

impl Document {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(
        id: impl Into<String>,
        content: impl Into<String>,
        metadata: HashMap<String, Value>,
    ) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata,
        }
    }
}
```

**Step 3: Verify all downstream tests pass**

```bash
cargo test -p synapse-retrieval && cargo test -p synapse-loaders
```

Expected: PASS. `Document::new(id, content)` still works. Tests only access `.id` and `.content` fields.

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add metadata field to Document"
```

---

### Task 10: Full Workspace Verification

**Files:** None (verification only)

**Step 1: Run full test suite**

```bash
cargo test --workspace
```

Expected: ALL tests pass across all crates and examples.

**Step 2: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: No warnings.

**Step 3: Run fmt check**

```bash
cargo fmt --all -- --check
```

Expected: No formatting issues.

**Step 4: Final commit if any fixes needed**

```bash
git add -A && git commit -m "chore: fix clippy warnings and formatting"
```

**Step 5: Tag Phase 1 complete**

```bash
git tag phase-1-core-refactor
```
