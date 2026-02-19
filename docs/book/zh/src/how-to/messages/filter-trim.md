# 过滤与裁剪消息

本指南展示如何从对话中筛选特定消息，以及如何裁剪消息列表以适应 token 预算。

## 使用 `filter_messages` 过滤消息

`filter_messages` 函数根据消息的类型（角色）、名称或 ID 筛选消息。它支持包含和排除两种过滤方式。

```rust
use synaptic_core::{filter_messages, Message};
```

### 按类型过滤

```rust
let messages = vec![
    Message::system("You are helpful."),
    Message::human("Question 1"),
    Message::ai("Answer 1"),
    Message::human("Question 2"),
    Message::ai("Answer 2"),
];

// Keep only human messages
let humans = filter_messages(
    &messages,
    Some(&["human"]),  // include_types
    None,              // exclude_types
    None,              // include_names
    None,              // exclude_names
    None,              // include_ids
    None,              // exclude_ids
);
assert_eq!(humans.len(), 2);
assert_eq!(humans[0].content(), "Question 1");
assert_eq!(humans[1].content(), "Question 2");
```

### 按类型排除

```rust
// Remove system messages, keep everything else
let without_system = filter_messages(
    &messages,
    None,                // include_types
    Some(&["system"]),   // exclude_types
    None, None, None, None,
);
assert_eq!(without_system.len(), 4);
```

### 按名称过滤

```rust
let messages = vec![
    Message::human("Hi").with_name("Alice"),
    Message::human("Hello").with_name("Bob"),
    Message::ai("Hey!"),
];

// Only messages from Alice
let alice_msgs = filter_messages(
    &messages,
    None, None,
    Some(&["Alice"]),  // include_names
    None, None, None,
);
assert_eq!(alice_msgs.len(), 1);
assert_eq!(alice_msgs[0].content(), "Hi");
```

### 按 ID 过滤

```rust
let messages = vec![
    Message::human("First").with_id("msg-1"),
    Message::human("Second").with_id("msg-2"),
    Message::human("Third").with_id("msg-3"),
];

// Exclude a specific message
let filtered = filter_messages(
    &messages,
    None, None, None, None,
    None,                         // include_ids
    Some(&["msg-2"]),             // exclude_ids
);
assert_eq!(filtered.len(), 2);
```

### 组合过滤条件

所有过滤参数可以组合使用。消息必须通过所有激活的过滤条件才会被包含：

```rust
// Keep only human messages from Alice
let result = filter_messages(
    &messages,
    Some(&["human"]),    // include_types
    None,                // exclude_types
    Some(&["Alice"]),    // include_names
    None, None, None,
);
```

## 使用 `trim_messages` 裁剪消息

`trim_messages` 函数将消息列表裁剪至 token 预算范围内。它支持两种策略：保留最前面的消息或保留最后面的消息。

```rust
use synaptic_core::{trim_messages, TrimStrategy, Message};
```

### 保留最后的消息（最常见）

这是聊天应用中的典型模式，用于保留最近的上下文：

```rust
let messages = vec![
    Message::system("You are a helpful assistant."),
    Message::human("Question 1"),
    Message::ai("Answer 1"),
    Message::human("Question 2"),
    Message::ai("Answer 2"),
    Message::human("Question 3"),
];

// Simple token counter: estimate ~4 chars per token
let token_counter = |msg: &Message| -> usize {
    msg.content().len() / 4
};

// Keep last messages within 50 tokens, preserve the system message
let trimmed = trim_messages(
    messages,
    50,               // max_tokens
    token_counter,
    TrimStrategy::Last,
    true,             // include_system: preserve the leading system message
);

// Result: system message + as many recent messages as fit in the budget
assert!(trimmed[0].is_system());
```

### 保留最前面的消息

当你需要保留对话开头的内容时使用：

```rust
let trimmed = trim_messages(
    messages,
    50,
    token_counter,
    TrimStrategy::First,
    false,  // include_system not relevant for First strategy
);
```

### `include_system` 参数

当使用 `TrimStrategy::Last` 且 `include_system: true` 时：

1. 如果第一条消息是系统消息，它将始终被保留。
2. 系统消息的 token 数会从预算中扣除。
3. 剩余预算从列表末尾开始填充消息。

这确保了即使对话增长，系统提示也不会被裁剪掉。

### 自定义 token 计数器

`token_counter` 参数是一个接受 `&Message` 并返回 `usize` token 计数的函数。你可以使用任何估算策略：

```rust
// Simple character-based estimate
let simple = |msg: &Message| -> usize { msg.content().len() / 4 };

// Word-based estimate
let word_based = |msg: &Message| -> usize {
    msg.content().split_whitespace().count()
};

// Fixed cost per message (useful when all messages are similar size)
let fixed = |_msg: &Message| -> usize { 10 };
```
