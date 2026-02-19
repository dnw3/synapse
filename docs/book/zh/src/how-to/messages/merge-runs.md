# 合并连续消息

本指南展示如何使用 `merge_message_runs` 将相同角色的连续消息合并为一条消息。

## 概述

一些 LLM 提供商要求消息角色交替出现（human、assistant、human、assistant）。如果你的消息历史中包含来自同一角色的连续消息，可以在发送请求之前将它们合并为一条。

## 基本用法

```rust
use synaptic_core::{merge_message_runs, Message};

let messages = vec![
    Message::human("Hello"),
    Message::human("How are you?"),       // Same role as previous
    Message::ai("I'm fine!"),
    Message::ai("Thanks for asking!"),    // Same role as previous
];

let merged = merge_message_runs(messages);

assert_eq!(merged.len(), 2);
assert_eq!(merged[0].content(), "Hello\nHow are you?");
assert_eq!(merged[1].content(), "I'm fine!\nThanks for asking!");
```

## 合并的工作原理

当两条连续消息具有相同角色时：

1. 它们的 `content` 字符串通过换行符（`\n`）连接。
2. 对于 AI 消息，后续消息的 `tool_calls` 和 `invalid_tool_calls` 会追加到第一条消息的列表中。
3. 合并后的消息保留该组中第一条消息的 `id`、`name` 和其他元数据。

## 合并带工具调用的 AI 消息

来自连续 AI 消息的工具调用会被合并：

```rust
use synaptic_core::{merge_message_runs, Message, ToolCall};
use serde_json::json;

let messages = vec![
    Message::ai_with_tool_calls("Looking up weather...", vec![
        ToolCall {
            id: "call_1".into(),
            name: "get_weather".into(),
            arguments: json!({"city": "Tokyo"}),
        },
    ]),
    Message::ai_with_tool_calls("Also checking news...", vec![
        ToolCall {
            id: "call_2".into(),
            name: "search_news".into(),
            arguments: json!({"query": "Tokyo"}),
        },
    ]),
];

let merged = merge_message_runs(messages);

assert_eq!(merged.len(), 1);
assert_eq!(merged[0].content(), "Looking up weather...\nAlso checking news...");
assert_eq!(merged[0].tool_calls().len(), 2);
```

## 保留不同角色

不同角色的消息永远不会被合并，即使它们看起来相关：

```rust
use synaptic_core::{merge_message_runs, Message};

let messages = vec![
    Message::system("Be helpful."),
    Message::human("Hi"),
    Message::ai("Hello!"),
    Message::human("Bye"),
];

let merged = merge_message_runs(messages);
assert_eq!(merged.len(), 4);  // No change -- all roles are different
```

## 实际用例：为提供商准备消息

一些提供商会拒绝包含连续同角色消息的请求。在发送之前使用 `merge_message_runs` 进行清理：

```rust
use synaptic_core::{merge_message_runs, ChatRequest, Message};

let conversation = vec![
    Message::system("You are a translator."),
    Message::human("Translate to French:"),
    Message::human("Hello, how are you?"),    // User sent two messages in a row
    Message::ai("Bonjour, comment allez-vous ?"),
];

let cleaned = merge_message_runs(conversation);
let request = ChatRequest::new(cleaned);
// Now safe to send: roles alternate correctly
```

## 空输入

`merge_message_runs` 在接收空输入时返回空向量：

```rust
use synaptic_core::merge_message_runs;

let result = merge_message_runs(vec![]);
assert!(result.is_empty());
```
