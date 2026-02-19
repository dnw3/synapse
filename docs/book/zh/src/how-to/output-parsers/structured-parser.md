# 结构化解析器

`StructuredOutputParser<T>` 将 JSON 字符串直接反序列化为类型化的 Rust 结构体。当你确切知道期望从 LLM 获得的数据形状时，这是首选的解析器。

## 基本用法

定义一个派生了 `Deserialize` 的结构体，然后为它创建解析器：

```rust
use synaptic_parsers::StructuredOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;
use serde::Deserialize;

#[derive(Deserialize)]
struct Person {
    name: String,
    age: u32,
}

let parser = StructuredOutputParser::<Person>::new();
let config = RunnableConfig::default();

let result = parser.invoke(
    r#"{"name": "Alice", "age": 30}"#.to_string(),
    &config,
).await?;

assert_eq!(result.name, "Alice");
assert_eq!(result.age, 30);
```

**签名：** `Runnable<String, T>`，其中 `T: DeserializeOwned + Send + Sync + 'static`

## 错误处理

如果输入字符串不是有效的 JSON 或不匹配结构体的 schema，解析器返回 `Err(SynapticError::Parsing(...))`：

```rust
use synaptic_parsers::StructuredOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    enabled: bool,
    threshold: f64,
}

let parser = StructuredOutputParser::<Config>::new();
let config = RunnableConfig::default();

// Missing required field -- returns an error
let err = parser.invoke(
    r#"{"enabled": true}"#.to_string(),
    &config,
).await.unwrap_err();

assert!(err.to_string().contains("structured parse error"));
```

## FormatInstructions

`StructuredOutputParser<T>` 实现了 `FormatInstructions` trait。在提示词中包含这些指令，以引导模型生成正确形状的 JSON：

```rust
use synaptic_parsers::{StructuredOutputParser, FormatInstructions};
use serde::Deserialize;

#[derive(Deserialize)]
struct Answer {
    reasoning: String,
    answer: String,
}

let parser = StructuredOutputParser::<Answer>::new();
let instructions = parser.get_format_instructions();
// "Your response should be a valid JSON object matching the expected schema."
```

## 管道示例

在链中，`StructuredOutputParser` 通常跟在 `StrOutputParser` 步骤之后，或直接接收字符串内容。以下是一个完整示例：

```rust
use synaptic_parsers::StructuredOutputParser;
use synaptic_runnables::{Runnable, RunnableLambda};
use synaptic_core::{Message, RunnableConfig};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Sentiment {
    label: String,
    confidence: f64,
}

// Simulate an LLM that returns JSON in a Message
let extract_content = RunnableLambda::new(|msg: Message| async move {
    Ok(msg.content().to_string())
});

let parser = StructuredOutputParser::<Sentiment>::new();

let chain = extract_content.boxed() | parser.boxed();
let config = RunnableConfig::default();

let input = Message::ai(r#"{"label": "positive", "confidence": 0.95}"#);
let result: Sentiment = chain.invoke(input, &config).await?;

assert_eq!(result.label, "positive");
assert!((result.confidence - 0.95).abs() < f64::EPSILON);
```

## 何时使用结构化解析器 vs. JSON 解析器

- 当你在编译时就知道确切的 schema 并且需要类型安全地访问字段时，使用 `StructuredOutputParser<T>`。
- 当你需要处理任意或动态的 JSON 结构（形状事先未知）时，使用 `JsonOutputParser`。
