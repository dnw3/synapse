# Token 计数与预算

Synaptic 提供了 token 计数和上下文预算基础组件，用于管理模型输入限制。

## TokenCounter Trait

`TokenCounter` trait 抽象了文本和消息的 token 计数。

```rust,ignore
use synaptic::core::TokenCounter;

pub trait TokenCounter: Send + Sync {
    fn count_text(&self, text: &str) -> usize;

    // 默认实现：sum(count_text(content) + 4) 每条消息的开销
    fn count_messages(&self, messages: &[Message]) -> usize;
}
```

### HeuristicTokenCounter

内置实现，按约 4 个字符对应 1 个 token 进行估算。

```rust,ignore
use synaptic::core::HeuristicTokenCounter;

let counter = HeuristicTokenCounter;
let tokens = counter.count_text("Hello, world!");  // 约 3 个 token
```

这是一种快速近似方法。如需精确计数，可使用 `tiktoken` 等分词器实现 `TokenCounter`。

## ContextBudget

`ContextBudget` 在 token 限制内，从多个具有优先级的槽位组装消息。

```rust,ignore
use synaptic::core::{ContextBudget, ContextSlot, Priority, HeuristicTokenCounter};

let counter = Arc::new(HeuristicTokenCounter);
let budget = ContextBudget::new(4096, counter);
```

### 优先级

槽位按优先级顺序处理。数值越小，优先级越高。

```rust,ignore
use synaptic::core::Priority;

Priority::CRITICAL  // 0 — 最先包含
Priority::HIGH      // 64
Priority::NORMAL    // 128
Priority::LOW       // 192 — 预算紧张时最先丢弃
```

### ContextSlot

每个槽位包含名称、优先级、消息列表和可选的保留 token 数。

```rust,ignore
use synaptic::core::ContextSlot;

let system_slot = ContextSlot {
    name: "system".to_string(),
    priority: Priority::CRITICAL,
    messages: vec![Message::system("You are a helpful assistant.")],
    reserved_tokens: 100,  // 若总保留量在预算内则保证包含
};

let history_slot = ContextSlot {
    name: "history".to_string(),
    priority: Priority::NORMAL,
    messages: conversation_history,
    reserved_tokens: 0,  // 尽力而为
};

let tool_slot = ContextSlot {
    name: "tool_results".to_string(),
    priority: Priority::HIGH,
    messages: tool_messages,
    reserved_tokens: 0,
};
```

### 组装预算

调用 `assemble()` 将槽位合并为符合预算的单一消息列表。

```rust,ignore
let messages = budget.assemble(vec![system_slot, history_slot, tool_slot]);
// 槽位按优先级排序。超出预算时低优先级槽位会被丢弃。
// 结果为扁平的 Vec<Message>。
```

高优先级槽位优先包含。如果某个槽位无法容纳且没有保留 token，则会被整体跳过。
