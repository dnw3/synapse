# Summary Buffer Memory

`ConversationSummaryBufferMemory` is a hybrid strategy that combines the strengths of [Summary Memory](summary.md) and [Token Buffer Memory](token-buffer.md). Recent messages are kept verbatim, while older messages are compressed into a running LLM-generated summary when the total estimated token count exceeds a configurable threshold.

## Usage

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationSummaryBufferMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message, ChatModel};

let model: Arc<dyn ChatModel> = Arc::new(my_model);
let store = Arc::new(InMemoryStore::new());

// Summarize older messages when total tokens exceed 500
let memory = ConversationSummaryBufferMemory::new(store, model, 500);

let session = "user-1";

memory.append(session, Message::human("What is Rust?")).await?;
memory.append(session, Message::ai("Rust is a systems programming language...")).await?;
memory.append(session, Message::human("How does ownership work?")).await?;
memory.append(session, Message::ai("Ownership is a set of rules...")).await?;
// ... as conversation grows and exceeds 500 estimated tokens,
// older messages are summarized automatically ...

let history = memory.load(session).await?;
// history = [System("Summary of earlier conversation: ..."), recent messages...]
```

## How It Works

1. **`append()`** stores the new message, then estimates the total token count across all stored messages.
2. When the total exceeds `max_token_limit` and there is more than one message:
   - A split point is calculated: recent messages that fit within half the token limit are kept verbatim.
   - All messages before the split point are summarized by the `ChatModel`. If a previous summary exists, it is included as context.
   - The store is cleared and repopulated with only the recent messages.
3. **`load()`** returns the stored messages, prepended with a system message containing the summary (if one exists):

   ```
   Summary of earlier conversation: <summary text>
   ```

4. **`clear()`** removes both stored messages and the summary for the session.

## Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `store` | `Arc<dyn MemoryStore>` | The backing store for raw messages |
| `model` | `Arc<dyn ChatModel>` | The LLM used to generate summaries |
| `max_token_limit` | `usize` | Token threshold that triggers summarization |

## Token Estimation

Like `ConversationTokenBufferMemory`, this strategy estimates tokens at approximately 4 characters per token (with a minimum of 1). The same heuristic caveat applies: actual token counts will vary by model.

## When to Use

Summary buffer memory is the recommended strategy when:

- Conversations are long and you need both exact recent context and compressed older context.
- You want to stay within a token budget while preserving as much information as possible.
- The additional cost of occasional LLM summarization calls is acceptable.

This is the closest equivalent to LangChain's `ConversationSummaryBufferMemory` and is generally the best default choice for production chatbots.

## Trade-offs

- **LLM cost on overflow** -- summarization only triggers when the token limit is exceeded, but each summarization call adds latency and cost.
- **Lossy for old messages** -- details from older messages may be lost in the summary, though recent messages are always exact.
- **Heuristic token counting** -- the split point is based on estimated tokens, not exact counts.

## Offline Testing with ScriptedChatModel

Use `ScriptedChatModel` to test summarization without API keys:

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, MemoryStore, Message};
use synaptic_models::ScriptedChatModel;
use synaptic_memory::{ConversationSummaryBufferMemory, InMemoryStore};

// Script the model to return a summary when called
let summarizer = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("The user asked about Rust and ownership."),
        usage: None,
    },
]));

let store = Arc::new(InMemoryStore::new());
let memory = ConversationSummaryBufferMemory::new(store, summarizer, 50);

let session = "test";

// Add enough messages to exceed the 50-token threshold
memory.append(session, Message::human("What is Rust?")).await?;
memory.append(session, Message::ai("Rust is a systems programming language focused on safety, speed, and concurrency.")).await?;
memory.append(session, Message::human("How does ownership work?")).await?;
memory.append(session, Message::ai("Ownership is a set of rules the compiler checks at compile time. Each value has a single owner.")).await?;

// Load -- older messages are now summarized
let history = memory.load(session).await?;
// history[0] is a System message with the summary
// Remaining messages are the most recent ones kept verbatim
```

For simpler alternatives, see [Buffer Memory](buffer.md) (keep everything), [Window Memory](window.md) (fixed message count), or [Token Buffer Memory](token-buffer.md) (token budget without summarization).
