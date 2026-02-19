# 流式传输

LLM 的响应可能需要数秒才能生成。如果没有流式传输，用户在整个响应完成之前什么都看不到。流式传输在 token 生成时即时传递，降低了感知延迟并支持实时 UI。本页解释流式传输在 Synaptic 各层中的工作方式——从单个模型调用到 LCEL 链再到 Graph 执行。

## 模型级流式传输

`ChatModel` trait 提供两个方法：

```rust
#[async_trait]
pub trait ChatModel: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError>;

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_>;
}
```

`chat()` 等待完整响应。`stream_chat()` 立即返回一个 `ChatStream`：

```rust
pub type ChatStream<'a> =
    Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send + 'a>>;
```

这是一个 pinned、boxed 的异步 `AIMessageChunk` 值流。每个 chunk 包含响应的一个片段——通常是几个 token 的文本、Tool 调用的一部分或使用量信息。

### 默认实现

`stream_chat()` 方法有一个默认实现，将 `chat()` 包装为单 chunk 流。如果模型适配器没有实现真正的流式传输，它会回退到这种行为——调用者仍然获得一个 Stream，但它只包含一个 chunk（完整响应）。这意味着消费 `ChatStream` 的代码可以与任何模型一起工作，无论它是否支持真正的流式传输。

### 消费 Stream

```rust
use futures::StreamExt;

let mut stream = model.stream_chat(request);

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);  // print tokens as they arrive
}
```

## AIMessageChunk 合并

流式传输会产生许多需要组装成完整消息的 chunk。`AIMessageChunk` 支持 `+` 和 `+=` 运算符：

```rust
let mut accumulated = AIMessageChunk::default();

while let Some(chunk) = stream.next().await {
    accumulated += chunk?;
}

let complete_message: Message = accumulated.into_message();
```

合并规则：
- **`content`**：通过 `push_str` 拼接。每个 chunk 的内容片段追加到累积的字符串上。
- **`tool_calls`**：扩展。chunk 可能携带部分或完整的 Tool 调用对象。
- **`tool_call_chunks`**：扩展。来自提供商的原始部分 Tool 调用数据。
- **`invalid_tool_calls`**：扩展。
- **`id`**：第一个非 `None` 的值优先。后续 chunk 不会覆盖 ID。
- **`usage`**：逐字段求和。如果两侧都有使用量数据，`input_tokens`、`output_tokens` 和 `total_tokens` 会相加。如果只有一侧有使用量数据，则保留该值。

累积完成后，`into_message()` 将 chunk 转换为包含完整内容和 Tool 调用的 `Message::AI`。

## LCEL 流式传输

`Runnable` trait 包含一个 `stream()` 方法：

```rust
fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>;
```

默认实现将 `invoke()` 包装为单元素流，类似于模型级的默认行为。支持真正流式传输的组件会重写此方法。

### 通过链进行流式传输

当你在 `BoxRunnable` 链（例如 `prompt | model | parser`）上调用 `stream()` 时，行为如下：

1. 中间步骤运行其 `invoke()` 方法并将结果向前传递。
2. 链中的**最后一个**组件流式输出其结果。

这意味着在 `prompt | model | parser` 链中，提示模板同步运行，模型真正流式传输，解析器在每个 chunk 到达时处理它（如果支持流式传输）或等待完整输出（如果不支持）。

```rust
let chain = prompt_template.boxed() | model_runnable.boxed() | parser.boxed();

let mut stream = chain.stream(input, &config);
while let Some(item) = stream.next().await {
    let output = item?;
    // Process each streamed output
}
```

### RunnableGenerator

对于生产自定义 Stream，`RunnableGenerator` 包装一个返回 Stream 的异步函数：

```rust
let generator = RunnableGenerator::new(|input: String, _config| {
    Box::pin(async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_string());
        }
    })
});
```

当你需要在 LCEL 链中注入一个非模型的流式数据源时，这非常有用。

## Graph 流式传输

Graph 执行也可以流式传输，在每个节点完成后产出事件：

```rust
use synaptic::graph::StreamMode;

let mut stream = graph.stream(initial_state, StreamMode::Values);

while let Some(event) = stream.next().await {
    let event = event?;
    println!("Node '{}' completed. Messages: {}", event.node, event.state.messages.len());
}
```

### StreamMode

| 模式 | 产出内容 | 适用场景 |
|------|--------|----------|
| `Values` | 每个节点执行后的完整状态 | 需要在每一步看到完整状态时 |
| `Updates` | 节点执行后的状态快照 | 需要观察每个节点改变了什么时 |

### GraphEvent

```rust
pub struct GraphEvent<S> {
    pub node: String,
    pub state: S,
}
```

每个事件告诉你哪个节点刚刚执行完毕以及状态是什么样的。对于 ReAct Agent，你会看到交替出现的 "agent" 和 "tools" 事件，消息在状态中不断累积。

## 何时使用流式传输

**使用模型级流式传输**：当你需要逐 token 输出用于聊天 UI，或者想在生成时向用户展示部分结果。

**使用 LCEL 流式传输**：当你有一个操作链并希望最终输出是流式的。中间步骤同步运行，但用户可以渐进地看到最终结果。

**使用 Graph 流式传输**：当你有一个多步骤工作流并希望观察进度。每个节点完成都是一个事件，让你能够了解 Graph 的执行情况。

## 流式传输与错误处理

Stream 可以在任何时刻产出错误。流式传输过程中的网络故障、来自提供商的格式错误的 chunk 或 Graph 节点故障都会在 Stream 中产生 `Err` 项。消费者应在每次 `next()` 调用时处理错误：

```rust
while let Some(result) = stream.next().await {
    match result {
        Ok(chunk) => process(chunk),
        Err(e) => {
            eprintln!("Stream error: {e}");
            break;
        }
    }
}
```

在 Stream 级别没有自动重试。如果 Stream 在中途失败，消费者决定如何处理——重试整个调用、返回部分结果或传播错误。如需自动重试，在流式传输之前将模型包装在 `RetryChatModel` 中，它会在失败时重试整个请求。

## 参见

- [ChatModel 流式传输](../how-to/chat-models/streaming.md) -- 模型级流式传输操作指南
- [LCEL 流式传输](../how-to/runnables/streaming.md) -- 通过 Runnable 链的流式传输
- [Graph 流式传输](../how-to/graph/streaming.md) -- 使用 StreamMode 的 Graph 级流式传输
- [Runnables 与 LCEL](runnables-lcel.md) -- Stream 运行所在的组合系统
