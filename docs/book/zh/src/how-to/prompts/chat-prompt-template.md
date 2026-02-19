# ChatPromptTemplate

`ChatPromptTemplate` 从一系列 `MessageTemplate` 条目生成 `Vec<Message>`。每个条目通过 `{{ variable }}` 插值渲染一条或多条消息。该模板实现了 `Runnable` trait，因此可以直接集成到 LCEL 管道中。

## 创建模板

使用 `ChatPromptTemplate::from_messages()`（或 `new()`）配合 `MessageTemplate` 变体的向量：

```rust
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a {{ role }} assistant."),
    MessageTemplate::human("{{ question }}"),
]);
```

## 使用 `format()` 渲染

使用 `HashMap<String, serde_json::Value>` 调用 `format()` 来生成消息：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a {{ role }} assistant."),
    MessageTemplate::human("{{ question }}"),
]);

let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("role".to_string(), json!("helpful")),
    ("question".to_string(), json!("What is Rust?")),
]);

let messages = template.format(&values).unwrap();
// messages[0] => Message::system("You are a helpful assistant.")
// messages[1] => Message::human("What is Rust?")
```

## 作为 Runnable 使用

由于 `ChatPromptTemplate` 实现了 `Runnable<HashMap<String, Value>, Vec<Message>>`，你可以调用 `invoke()` 或使用管道运算符进行组合：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_core::RunnableConfig;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic_runnables::Runnable;

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a {{ role }} assistant."),
    MessageTemplate::human("{{ question }}"),
]);

let config = RunnableConfig::default();
let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("role".to_string(), json!("helpful")),
    ("question".to_string(), json!("What is Rust?")),
]);

let messages = template.invoke(values, &config).await?;
// messages = [Message::system("You are a helpful assistant."), Message::human("What is Rust?")]
```

## MessageTemplate 变体

`MessageTemplate` 是一个包含四个变体的枚举：

| 变体 | 描述 |
|---------|-------------|
| `MessageTemplate::system(text)` | 从模板字符串渲染系统消息 |
| `MessageTemplate::human(text)` | 从模板字符串渲染用户消息 |
| `MessageTemplate::ai(text)` | 从模板字符串渲染 AI 消息 |
| `MessageTemplate::Placeholder(key)` | 从输入映射中注入消息列表 |

### Placeholder 示例

`Placeholder` 注入存储在输入映射中某个键下的消息。该值必须是序列化的 `Message` 对象的 JSON 数组。这对于注入对话历史非常有用：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are helpful."),
    MessageTemplate::Placeholder("history".to_string()),
    MessageTemplate::human("{{ input }}"),
]);

let history = json!([
    {"role": "human", "content": "Hi"},
    {"role": "assistant", "content": "Hello!"}
]);

let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("history".to_string(), history),
    ("input".to_string(), json!("How are you?")),
]);

let messages = template.format(&values).unwrap();
// messages[0] => System("You are helpful.")
// messages[1] => Human("Hi")         -- from placeholder
// messages[2] => AI("Hello!")         -- from placeholder
// messages[3] => Human("How are you?")
```

## 在管道中组合

一种常见模式是将提示词模板传入聊天模型，再传入 OutputParser：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_core::{ChatModel, ChatResponse, Message, RunnableConfig};
use synaptic_models::ScriptedChatModel;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;

let model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Rust is a systems programming language."),
        usage: None,
    },
]);

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a {{ role }} assistant."),
    MessageTemplate::human("{{ question }}"),
]);

let chain = template.boxed() | model.boxed() | StrOutputParser.boxed();

let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("role".to_string(), json!("helpful")),
    ("question".to_string(), json!("What is Rust?")),
]);

let config = RunnableConfig::default();
let result: String = chain.invoke(values, &config).await.unwrap();
// result = "Rust is a systems programming language."
```
