# Memory

Without memory, every LLM call is stateless -- the model has no knowledge of previous interactions. Memory in Synaptic solves this by storing, retrieving, and managing conversation history so that subsequent calls include relevant context. This page explains the memory abstraction, the available strategies, and how they trade off between completeness and cost.

## The MemoryStore Trait

All memory backends implement a single trait:

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError>;
    async fn clear(&self, session_id: &str) -> Result<(), SynapticError>;
}
```

Three operations, keyed by a session identifier:
- **`append`** -- add a message to the session's history
- **`load`** -- retrieve the full history for a session
- **`clear`** -- delete all messages for a session

The `session_id` parameter is central to Synaptic's memory design. Two conversations with different session IDs are completely isolated, even if they share the same memory store instance. This enables multi-tenant applications where many users interact concurrently through a single system.

## InMemoryStore

The simplest implementation -- a `HashMap<String, Vec<Message>>` wrapped in `Arc<RwLock<_>>`:

```rust
use synaptic::memory::InMemoryStore;

let store = InMemoryStore::new();
store.append("session_1", Message::human("Hello")).await?;
let history = store.load("session_1").await?;
```

`InMemoryStore` is fast, requires no external dependencies, and is suitable for development, testing, and short-lived applications. Data is lost when the process exits.

## FileChatMessageHistory

A persistent store that writes messages to a JSON file on disk. Each session is stored as a separate file. This is useful for applications that need persistence without a database:

```rust
use synaptic::memory::FileChatMessageHistory;

let history = FileChatMessageHistory::new("./chat_history")?;
```

## Memory Strategies

Raw `MemoryStore` keeps every message forever. For long conversations, this leads to unbounded token usage and eventually exceeds the model's context window. Memory strategies wrap a store and control which messages are included in the context.

### ConversationBufferMemory

Keeps all messages. The simplest strategy -- everything is sent to the LLM every time.

- **Advantage**: No information loss.
- **Disadvantage**: Token usage grows without bound. Eventually exceeds the context window.
- **Use case**: Short conversations where you know the total message count is small.

### ConversationWindowMemory

Keeps only the last K message pairs (human + AI). Older messages are dropped:

```rust
use synaptic::memory::ConversationWindowMemory;

let memory = ConversationWindowMemory::new(store, 5); // keep last 5 exchanges
```

- **Advantage**: Fixed, predictable token usage.
- **Disadvantage**: Complete loss of older context. The model has no knowledge of what happened more than K turns ago.
- **Use case**: Chat UIs, customer service bots, and any scenario where recent context matters most.

### ConversationSummaryMemory

Summarizes older messages using an LLM, keeping only the summary plus recent messages:

```rust
use synaptic::memory::ConversationSummaryMemory;

let memory = ConversationSummaryMemory::new(store, summarizer_model);
```

After each exchange, the strategy uses an LLM to produce a running summary of the conversation. The summary replaces the older messages, so the context sent to the main model includes the summary followed by recent messages.

- **Advantage**: Retains the gist of the entire conversation. Constant-ish token usage.
- **Disadvantage**: Summarization has a cost (an extra LLM call). Details may be lost in compression. Summarization quality depends on the model.
- **Use case**: Long-running conversations where historical context matters (e.g., a multi-session assistant that remembers past preferences).

### ConversationTokenBufferMemory

Keeps as many recent messages as fit within a token budget:

```rust
use synaptic::memory::ConversationTokenBufferMemory;

let memory = ConversationTokenBufferMemory::new(store, 4096); // max 4096 tokens
```

Unlike window memory (which counts messages), token buffer memory counts tokens. This is more precise when messages vary significantly in length.

- **Advantage**: Direct control over context size. Works well with models that have strict context limits.
- **Disadvantage**: Still loses old messages entirely.
- **Use case**: Cost-sensitive applications where you want to fill the context window efficiently.

### ConversationSummaryBufferMemory

A hybrid: summarizes old messages and keeps recent ones, with a token threshold controlling the boundary:

```rust
use synaptic::memory::ConversationSummaryBufferMemory;

let memory = ConversationSummaryBufferMemory::new(store, model, 2000);
// Summarize when recent messages exceed 2000 tokens
```

When the total token count of recent messages exceeds the threshold, the oldest messages are summarized and replaced with the summary. The result is a context that starts with a summary of the distant past, followed by verbatim recent messages.

- **Advantage**: Best of both worlds -- retains old context through summaries while keeping recent messages verbatim.
- **Disadvantage**: More complex. Requires an LLM for summarization.
- **Use case**: Production chat applications that need both historical awareness and accurate recent context.

## Strategy Comparison

| Strategy | What It Keeps | Token Growth | Info Loss | Extra LLM Calls |
|----------|---------------|-------------|-----------|-----------------|
| Buffer | Everything | Unbounded | None | None |
| Window | Last K turns | Fixed | Old messages lost | None |
| Summary | Summary + recent | Near-constant | Details compressed | Yes |
| TokenBuffer | Recent within budget | Fixed | Old messages lost | None |
| SummaryBuffer | Summary + recent buffer | Bounded | Old details compressed | Yes |

## RunnableWithMessageHistory

Rather than manually loading and saving messages around each LLM call, `RunnableWithMessageHistory` wraps any `Runnable` and handles it automatically:

```rust
use synaptic::memory::RunnableWithMessageHistory;

let chain_with_memory = RunnableWithMessageHistory::new(
    my_chain,
    store,
    |config| config.metadata.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string(),
);
```

On each invocation:
1. The session ID is extracted from the `RunnableConfig` metadata.
2. Historical messages are loaded from the store.
3. The inner runnable is invoked with the historical context prepended.
4. The new messages (input and output) are appended to the store.

This separates memory management from application logic. The inner runnable does not need to know about memory at all.

## Session Isolation

A key design property: memory is always scoped to a session. The `session_id` is just a string -- it could be a user ID, a conversation ID, a thread ID, or any other identifier meaningful to your application.

Different sessions sharing the same `InMemoryStore` (or any other store) are completely independent. Appending to session "alice" never affects session "bob". This makes it safe to use a single store instance across an entire application serving multiple users.

## See Also

- [Buffer Memory](../how-to/memory/buffer.md) -- keeping all messages
- [Window Memory](../how-to/memory/window.md) -- keeping last K turns
- [Summary Memory](../how-to/memory/summary.md) -- LLM-based summarization
- [Token Buffer Memory](../how-to/memory/token-buffer.md) -- token-budget trimming
- [Summary Buffer Memory](../how-to/memory/summary-buffer.md) -- hybrid summary + recent buffer
- [RunnableWithMessageHistory](../how-to/memory/runnable-with-history.md) -- automatic history management
- [Messages](messages.md) -- the Message type that memory stores
