# Token Counting & Budget

Synaptic provides token counting and context budget primitives for managing model input limits.

## TokenCounter Trait

The `TokenCounter` trait abstracts token counting for text and messages.

```rust,ignore
use synaptic::core::TokenCounter;

pub trait TokenCounter: Send + Sync {
    fn count_text(&self, text: &str) -> usize;

    // Default: sum of count_text(content) + 4 per-message overhead
    fn count_messages(&self, messages: &[Message]) -> usize;
}
```

### HeuristicTokenCounter

A built-in implementation that estimates ~4 characters per token.

```rust,ignore
use synaptic::core::HeuristicTokenCounter;

let counter = HeuristicTokenCounter;
let tokens = counter.count_text("Hello, world!");  // ~3 tokens
```

This is a fast approximation. For precise counting, implement `TokenCounter` with a tokenizer such as `tiktoken`.

## ContextBudget

`ContextBudget` assembles messages from multiple prioritized slots within a token limit.

```rust,ignore
use synaptic::core::{ContextBudget, ContextSlot, Priority, HeuristicTokenCounter};

let counter = Arc::new(HeuristicTokenCounter);
let budget = ContextBudget::new(4096, counter);
```

### Priority Levels

Slots are processed in priority order. Lower values mean higher priority.

```rust,ignore
use synaptic::core::Priority;

Priority::CRITICAL  // 0 — always included first
Priority::HIGH      // 64
Priority::NORMAL    // 128
Priority::LOW       // 192 — dropped first when budget is tight
```

### ContextSlot

Each slot carries a name, priority, messages, and an optional reserved token count.

```rust,ignore
use synaptic::core::ContextSlot;

let system_slot = ContextSlot {
    name: "system".to_string(),
    priority: Priority::CRITICAL,
    messages: vec![Message::system("You are a helpful assistant.")],
    reserved_tokens: 100,  // guaranteed if total reserved fits
};

let history_slot = ContextSlot {
    name: "history".to_string(),
    priority: Priority::NORMAL,
    messages: conversation_history,
    reserved_tokens: 0,  // best-effort
};

let tool_slot = ContextSlot {
    name: "tool_results".to_string(),
    priority: Priority::HIGH,
    messages: tool_messages,
    reserved_tokens: 0,
};
```

### Assembling the Budget

Call `assemble()` to merge slots into a single message list that fits the budget.

```rust,ignore
let messages = budget.assemble(vec![system_slot, history_slot, tool_slot]);
// Slots are sorted by priority. Lower-priority slots are dropped if
// the budget is exceeded. The result is a flat Vec<Message>.
```

Higher-priority slots are included first. If a slot does not fit and has no reserved tokens, it is skipped entirely.
