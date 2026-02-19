# Streaming

LLM responses can take seconds to generate. Without streaming, the user sees nothing until the entire response is complete. Streaming delivers tokens as they are produced, reducing perceived latency and enabling real-time UIs. This page explains how streaming works across Synaptic's layers -- from individual model calls through LCEL chains to graph execution.

## Model-Level Streaming

The `ChatModel` trait provides two methods:

```rust
#[async_trait]
pub trait ChatModel: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError>;

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_>;
}
```

`chat()` waits for the complete response. `stream_chat()` returns a `ChatStream` immediately:

```rust
pub type ChatStream<'a> =
    Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send + 'a>>;
```

This is a pinned, boxed, async stream of `AIMessageChunk` values. Each chunk contains a fragment of the response -- typically a few tokens of text, part of a tool call, or usage information.

### Default Implementation

The `stream_chat()` method has a default implementation that wraps `chat()` as a single-chunk stream. If a model adapter does not implement true streaming, it falls back to this behavior -- the caller still gets a stream, but it contains only one chunk (the complete response). This means code that consumes a `ChatStream` works with any model, whether or not it supports true streaming.

### Consuming a Stream

```rust
use futures::StreamExt;

let mut stream = model.stream_chat(request);

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);  // print tokens as they arrive
}
```

## AIMessageChunk Merging

Streaming produces many chunks that must be assembled into a complete message. `AIMessageChunk` supports the `+` and `+=` operators:

```rust
let mut accumulated = AIMessageChunk::default();

while let Some(chunk) = stream.next().await {
    accumulated += chunk?;
}

let complete_message: Message = accumulated.into_message();
```

The merge rules:
- **`content`**: Concatenated via `push_str`. Each chunk's content fragment is appended to the accumulated string.
- **`tool_calls`**: Extended. Chunks may carry partial or complete tool call objects.
- **`tool_call_chunks`**: Extended. Raw partial tool call data from the provider.
- **`invalid_tool_calls`**: Extended.
- **`id`**: The first non-`None` value wins. Subsequent chunks do not overwrite the ID.
- **`usage`**: Summed field-by-field. If both sides have usage data, `input_tokens`, `output_tokens`, and `total_tokens` are added together. If only one side has usage, it is preserved.

After accumulation, `into_message()` converts the chunk into a `Message::AI` with the complete content and tool calls.

## LCEL Streaming

The `Runnable` trait includes a `stream()` method:

```rust
fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>;
```

The default implementation wraps `invoke()` as a single-item stream, similar to the model-level default. Components that support true streaming override this method.

### Streaming Through Chains

When you call `stream()` on a `BoxRunnable` chain (e.g., `prompt | model | parser`), the behavior is:

1. Intermediate steps run their `invoke()` method and pass the result forward.
2. The **final** component in the chain streams its output.

This means in a `prompt | model | parser` chain, the prompt template runs synchronously, the model truly streams, and the parser processes each chunk as it arrives (if it supports streaming) or waits for the complete output (if it does not).

```rust
let chain = prompt_template.boxed() | model_runnable.boxed() | parser.boxed();

let mut stream = chain.stream(input, &config);
while let Some(item) = stream.next().await {
    let output = item?;
    // Process each streamed output
}
```

### RunnableGenerator

For producing custom streams, `RunnableGenerator` wraps an async function that returns a stream:

```rust
let generator = RunnableGenerator::new(|input: String, _config| {
    Box::pin(async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_string());
        }
    })
});
```

This is useful when you need to inject a streaming source into an LCEL chain that is not a model.

## Graph Streaming

Graph execution can also stream, yielding events after each node completes:

```rust
use synaptic::graph::StreamMode;

let mut stream = graph.stream(initial_state, StreamMode::Values);

while let Some(event) = stream.next().await {
    let event = event?;
    println!("Node '{}' completed. Messages: {}", event.node, event.state.messages.len());
}
```

### StreamMode

| Mode | Yields | Use Case |
|------|--------|----------|
| `Values` | Full state after each node | When you need the complete picture at each step |
| `Updates` | Post-node state snapshot | When you want to observe what each node changed |

### GraphEvent

```rust
pub struct GraphEvent<S> {
    pub node: String,
    pub state: S,
}
```

Each event tells you which node just executed and what the state looks like. For a ReAct agent, you would see alternating "agent" and "tools" events, with messages accumulating in the state.

## When to Use Streaming

**Use model-level streaming** when you need token-by-token output for a chat UI or when you want to show partial results to the user as they are generated.

**Use LCEL streaming** when you have a chain of operations and want the final output to stream. The intermediate steps run synchronously, but the user sees the final result incrementally.

**Use graph streaming** when you have a multi-step workflow and want to observe progress. Each node completion is an event, giving you visibility into the graph's execution.

## Streaming and Error Handling

Streams can yield errors at any point. A network failure mid-stream, a malformed chunk from the provider, or a graph node failure all produce `Err` items in the stream. Consumers should handle errors on each `next()` call:

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

There is no automatic retry at the stream level. If a stream fails mid-way, the consumer decides how to handle it -- retry the entire call, return a partial result, or propagate the error. For automatic retries, wrap the model in a `RetryChatModel` before streaming, which retries the entire request on failure.

## See Also

- [Chat Model Streaming](../how-to/chat-models/streaming.md) -- model-level streaming how-to
- [LCEL Streaming](../how-to/runnables/streaming.md) -- streaming through runnable chains
- [Graph Streaming](../how-to/graph/streaming.md) -- graph-level streaming with StreamMode
- [Runnables & LCEL](runnables-lcel.md) -- the composition system that streams run through
