# 枚举解析器

`EnumOutputParser` 验证 LLM 的输出是否匹配一组预定义的允许值。这对于分类任务非常有用，在这类任务中模型应该精确地回复若干类别之一。

## 基本用法

使用允许值列表创建解析器，然后调用它：

```rust
use synaptic_parsers::EnumOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = EnumOutputParser::new(vec![
    "positive".to_string(),
    "negative".to_string(),
    "neutral".to_string(),
]);

let config = RunnableConfig::default();

// Valid value -- returns Ok
let result = parser.invoke("positive".to_string(), &config).await?;
assert_eq!(result, "positive");
```

**签名：** `Runnable<String, String>`

## 验证

解析器在检查前会去除输入的空白。如果去除空白后的输入不匹配任何允许值，则返回 `Err(SynapticError::Parsing(...))`：

```rust
use synaptic_parsers::EnumOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = EnumOutputParser::new(vec![
    "positive".to_string(),
    "negative".to_string(),
    "neutral".to_string(),
]);

let config = RunnableConfig::default();

// Whitespace is trimmed -- this succeeds
let result = parser.invoke("  neutral  ".to_string(), &config).await?;
assert_eq!(result, "neutral");

// Invalid value -- returns an error
let err = parser.invoke("invalid".to_string(), &config).await.unwrap_err();
assert!(err.to_string().contains("expected one of"));
```

## FormatInstructions

`EnumOutputParser` 实现了 `FormatInstructions`。在提示词中包含这些指令，让模型知道可以从哪些值中选择：

```rust
use synaptic_parsers::{EnumOutputParser, FormatInstructions};

let parser = EnumOutputParser::new(vec![
    "positive".to_string(),
    "negative".to_string(),
    "neutral".to_string(),
]);

let instructions = parser.get_format_instructions();
// "Your response should be one of the following values: positive, negative, neutral"
```

## 管道示例

一个典型的分类管道组合了提示词、模型、内容提取器和枚举解析器：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_core::{ChatResponse, Message, RunnableConfig};
use synaptic_models::ScriptedChatModel;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic_parsers::{StrOutputParser, EnumOutputParser, FormatInstructions};
use synaptic_runnables::Runnable;

let parser = EnumOutputParser::new(vec![
    "positive".to_string(),
    "negative".to_string(),
    "neutral".to_string(),
]);

let model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("positive"),
        usage: None,
    },
]);

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system(
        &format!(
            "Classify the sentiment of the text. {}",
            parser.get_format_instructions()
        ),
    ),
    MessageTemplate::human("{{ text }}"),
]);

// template -> model -> extract content -> validate enum
let chain = template.boxed()
    | model.boxed()
    | StrOutputParser.boxed()
    | parser.boxed();

let config = RunnableConfig::default();
let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("text".to_string(), json!("I love this product!")),
]);

let result: String = chain.invoke(values, &config).await?;
assert_eq!(result, "positive");
```
