# Structured Output

本指南展示如何使用 `StructuredOutputChatModel<T>` 从 LLM 响应中获取类型化的 Rust 结构体。

## 概述

`StructuredOutputChatModel<T>` 包装任意 `ChatModel`，指示其以符合您描述的 schema 的有效 JSON 进行响应。它会注入包含 schema 说明的系统提示，并提供 `parse_response()` 方法将 JSON 反序列化为您的 Rust 类型。

## 基本用法

将输出类型定义为实现了 `Deserialize` 的结构体，然后包装您的模型：

```rust
use std::sync::Arc;
use serde::Deserialize;
use synaptic_core::{ChatModel, ChatRequest, Message};
use synaptic_models::StructuredOutputChatModel;

#[derive(Debug, Deserialize)]
struct MovieReview {
    title: String,
    rating: f32,
    summary: String,
}

async fn get_review(base_model: Arc<dyn ChatModel>) -> Result<(), Box<dyn std::error::Error>> {
    let structured = StructuredOutputChatModel::<MovieReview>::new(
        base_model,
        r#"{"title": "string", "rating": "number (1-10)", "summary": "string"}"#,
    );

    let request = ChatRequest::new(vec![
        Message::human("Review the movie 'Interstellar'"),
    ]);

    // Use generate() to get both the parsed struct and the raw response
    let (review, _raw_response) = structured.generate(request).await?;

    println!("Title: {}", review.title);
    println!("Rating: {}/10", review.rating);
    println!("Summary: {}", review.summary);

    Ok(())
}
```

## 工作原理

当您在 `StructuredOutputChatModel` 上调用 `chat()` 或 `generate()` 时：

1. 在请求前添加一条系统消息，指示模型以匹配 schema 描述的有效 JSON 进行响应。
2. 请求被转发给内部模型。
3. 使用 `generate()` 时，响应文本会被解析为 JSON 并转换为目标类型 `T`。

schema 描述是自由格式的字符串。它不需要是有效的 JSON Schema——只需要清楚地向 LLM 传达预期的数据结构即可：

```rust
// Simple field descriptions
let schema = r#"{"name": "string", "age": "integer", "hobbies": ["string"]}"#;

// More detailed descriptions
let schema = r#"{
    "sentiment": "one of: positive, negative, neutral",
    "confidence": "float between 0.0 and 1.0",
    "key_phrases": "array of strings"
}"#;
```

## 手动解析响应

如果您想将模型当作普通 `ChatModel` 使用，稍后再解析，可以先调用 `chat()` 再调用 `parse_response()`：

```rust
let structured = StructuredOutputChatModel::<MovieReview>::new(base_model, schema);

let response = structured.chat(request).await?;
let parsed: MovieReview = structured.parse_response(&response)?;
```

## 处理 Markdown 代码块

解析器会自动处理包裹在 Markdown 代码块中的响应。以下格式均受支持：

```text
{"title": "Interstellar", "rating": 9.0, "summary": "..."}
```

````text
```json
{"title": "Interstellar", "rating": 9.0, "summary": "..."}
```
````

````text
```
{"title": "Interstellar", "rating": 9.0, "summary": "..."}
```
````

## 复杂输出类型

您可以使用嵌套结构体、枚举和集合：

```rust
#[derive(Debug, Deserialize)]
struct AnalysisResult {
    entities: Vec<Entity>,
    sentiment: Sentiment,
    language: String,
}

#[derive(Debug, Deserialize)]
struct Entity {
    name: String,
    entity_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Sentiment {
    Positive,
    Negative,
    Neutral,
}

let structured = StructuredOutputChatModel::<AnalysisResult>::new(
    base_model,
    r#"{
        "entities": [{"name": "string", "entity_type": "person|org|location"}],
        "sentiment": "positive|negative|neutral",
        "language": "ISO 639-1 code"
    }"#,
);
```

## 与其他包装器组合

由于 `StructuredOutputChatModel<T>` 实现了 `ChatModel`，它可以与其他包装器组合使用：

```rust
use synaptic_models::{RetryChatModel, RetryPolicy};

let base: Arc<dyn ChatModel> = Arc::new(base_model);
let structured = Arc::new(StructuredOutputChatModel::<MovieReview>::new(
    base,
    r#"{"title": "string", "rating": "number", "summary": "string"}"#,
));

// Add retry logic on top
let reliable = RetryChatModel::new(structured, RetryPolicy::default());
```
