# 提示词

Synaptic 提供两个层级的提示词模板：

- **`PromptTemplate`** -- 使用 `{{ variable }}` 语法的简单字符串插值。接受 `HashMap<String, String>` 并返回渲染后的 `String`。
- **`ChatPromptTemplate`** -- 从一系列 `MessageTemplate` 条目生成 `Vec<Message>`。每个条目可以是系统、用户或 AI 消息模板，也可以是注入现有消息列表的 `Placeholder`。

两种模板类型都实现了 `Runnable` trait，因此它们可以使用 LCEL 管道运算符（`|`）直接与聊天模型、OutputParser 和其他 Runnable 组合。

## 快速示例

```rust
use synaptic_prompts::{PromptTemplate, ChatPromptTemplate, MessageTemplate};

// Simple string template
let pt = PromptTemplate::new("Hello, {{ name }}!");
let mut values = std::collections::HashMap::new();
values.insert("name".to_string(), "world".to_string());
assert_eq!(pt.render(&values).unwrap(), "Hello, world!");

// Chat message template (produces Vec<Message>)
let chat = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a {{ role }} assistant."),
    MessageTemplate::human("{{ question }}"),
]);
```

## 子页面

- [ChatPromptTemplate](chat-prompt-template.md) -- 使用变量插值和占位符构建多消息提示词
- [Few-Shot 提示](few-shot.md) -- 注入示例对话以实现少样本学习
