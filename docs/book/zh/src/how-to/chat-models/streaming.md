# 流式响应

本指南展示如何以 token 流的方式消费 LLM 响应，而无需等待整个响应生成完毕。

## 概述

Synaptic 中每个 `ChatModel` 都提供两个方法：

- `chat()` -- 模型完成生成后返回完整的 `ChatResponse`。
- `stream_chat()` -- 返回 `ChatStream`，在模型生成过程中逐步产出 `AIMessageChunk`。

流式输出适用于向用户实时展示部分结果的场景。

## 基本流式用法

使用 `stream_chat()` 并通过 `StreamExt::next()` 迭代各个 chunk：

```rust
use futures::StreamExt;
use synaptic_core::{ChatModel, ChatRequest, Message, AIMessageChunk};

async fn stream_example(model: &dyn ChatModel) -> Result<(), Box<dyn std::error::Error>> {
    let request = ChatRequest::new(vec![
        Message::human("Tell me a story about a brave robot"),
    ]);

    let mut stream = model.stream_chat(request);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.content);  // Print each token as it arrives
    }
    println!();  // Final newline

    Ok(())
}
```

`ChatStream` 类型定义如下：

```rust
type ChatStream<'a> = Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send + 'a>>;
```

## 将 chunk 累积为完整消息

`AIMessageChunk` 支持 `+` 和 `+=` 运算符，用于合并多个 chunk。流式传输完成后，可将累积结果转换为完整的 `Message`：

```rust
use futures::StreamExt;
use synaptic_core::{ChatModel, ChatRequest, Message, AIMessageChunk};

async fn accumulate_stream(model: &dyn ChatModel) -> Result<Message, Box<dyn std::error::Error>> {
    let request = ChatRequest::new(vec![
        Message::human("Summarize Rust's ownership model"),
    ]);

    let mut stream = model.stream_chat(request);
    let mut full = AIMessageChunk::default();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        full += chunk;  // Merge content, tool_calls, usage, etc.
    }

    let final_message = full.into_message();
    println!("Complete response: {}", final_message.content());

    Ok(final_message)
}
```

合并 chunk 时：
- `content` 字符串会拼接在一起。
- `tool_calls` 会追加到累积列表中。
- `usage` 中的 token 计数会求和。
- 保留第一个非 `None` 的 `id`。

## 使用 `+` 运算符

也可以使用 `+` 在不修改原值的情况下组合两个 chunk：

```rust
let combined = chunk_a + chunk_b;
```

这会生成一个新的 `AIMessageChunk`，包含两者合并后的字段。

## 带 Tool 调用的流式传输

当模型流式返回的响应包含 Tool 调用时，Tool 调用数据会分散在多个 chunk 中。累积完成后，完整的 Tool 调用信息可从结果消息中获取：

```rust
use futures::StreamExt;
use synaptic_core::{ChatModel, ChatRequest, Message, AIMessageChunk, ToolDefinition};
use serde_json::json;

async fn stream_with_tools(model: &dyn ChatModel) -> Result<(), Box<dyn std::error::Error>> {
    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get current weather".to_string(),
        parameters: json!({"type": "object", "properties": {"city": {"type": "string"}}}),
    };

    let request = ChatRequest::new(vec![
        Message::human("What's the weather in Paris?"),
    ]).with_tools(vec![tool]);

    let mut stream = model.stream_chat(request);
    let mut full = AIMessageChunk::default();

    while let Some(chunk) = stream.next().await {
        full += chunk?;
    }

    let message = full.into_message();
    for tc in message.tool_calls() {
        println!("Call tool '{}' with: {}", tc.name, tc.arguments);
    }

    Ok(())
}
```

## 默认流式行为

如果提供商适配器未实现原生流式传输，默认的 `stream_chat()` 实现会将 `chat()` 的结果包装为单个 chunk 的流。这意味着无论提供商是否支持，您都可以使用 `stream_chat()`——只是对于不原生支持流式传输的提供商，您不会获得逐 token 的增量传输。
