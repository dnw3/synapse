# Error Handling

Synaptic uses a single error enum, `SynapticError`, across the entire framework. Every async function returns `Result<T, SynapticError>`, and errors propagate naturally with the `?` operator. This page explains the error model, the available variants, and the patterns for handling and recovering from errors.

## SynapticError

```rust
#[derive(Debug, Error)]
pub enum SynapticError {
    #[error("prompt error: {0}")]           Prompt(String),
    #[error("model error: {0}")]            Model(String),
    #[error("tool error: {0}")]             Tool(String),
    #[error("tool not found: {0}")]         ToolNotFound(String),
    #[error("memory error: {0}")]           Memory(String),
    #[error("rate limit: {0}")]             RateLimit(String),
    #[error("timeout: {0}")]                Timeout(String),
    #[error("validation error: {0}")]       Validation(String),
    #[error("parsing error: {0}")]          Parsing(String),
    #[error("callback error: {0}")]         Callback(String),
    #[error("max steps exceeded: {max_steps}")]  MaxStepsExceeded { max_steps: usize },
    #[error("embedding error: {0}")]        Embedding(String),
    #[error("vector store error: {0}")]     VectorStore(String),
    #[error("retriever error: {0}")]        Retriever(String),
    #[error("loader error: {0}")]           Loader(String),
    #[error("splitter error: {0}")]         Splitter(String),
    #[error("graph error: {0}")]            Graph(String),
    #[error("cache error: {0}")]            Cache(String),
    #[error("config error: {0}")]           Config(String),
    #[error("mcp error: {0}")]             Mcp(String),
}
```

Twenty variants, one for each subsystem. The design is intentional:

- **Single type everywhere**: You never need to convert between error types. Any function in any crate can return `SynapticError`, and the caller can propagate it with `?` without conversion.
- **String payloads**: Most variants carry a `String` message. This keeps the error type simple and avoids nested error hierarchies. The message provides context about what went wrong.
- **`thiserror` derivation**: `SynapticError` implements `std::error::Error` and `Display` automatically via the `#[error(...)]` attributes.

## Variant Reference

### Infrastructure Errors

| Variant | When It Occurs |
|---------|----------------|
| `Model(String)` | LLM provider returns an error, network failure, invalid response format |
| `RateLimit(String)` | Provider rate limit exceeded, token bucket exhausted |
| `Timeout(String)` | Request timed out |
| `Config(String)` | Invalid configuration (missing API key, bad parameters) |

### Input/Output Errors

| Variant | When It Occurs |
|---------|----------------|
| `Prompt(String)` | Template variable missing, invalid template syntax |
| `Validation(String)` | Input fails validation (e.g., empty message list, invalid schema) |
| `Parsing(String)` | Output parser cannot extract structured data from LLM response |

### Tool Errors

| Variant | When It Occurs |
|---------|----------------|
| `Tool(String)` | Tool execution failed (network error, computation error, etc.) |
| `ToolNotFound(String)` | Requested tool name is not in the registry |

### Subsystem Errors

| Variant | When It Occurs |
|---------|----------------|
| `Memory(String)` | Memory store read/write failure |
| `Callback(String)` | Callback handler raised an error |
| `Embedding(String)` | Embedding API failure |
| `VectorStore(String)` | Vector store read/write failure |
| `Retriever(String)` | Retrieval operation failed |
| `Loader(String)` | Document loading failed (file not found, parse error) |
| `Splitter(String)` | Text splitting failed |
| `Cache(String)` | Cache read/write failure |

### Execution Control Errors

| Variant | When It Occurs |
|---------|----------------|
| `Graph(String)` | Graph execution error (compilation, routing, missing nodes) |
| `MaxStepsExceeded { max_steps }` | Agent loop exceeded the maximum iteration count |
| `Mcp(String)` | MCP server connection, transport, or protocol error |

## Error Propagation

Because every async function in Synaptic returns `Result<T, SynapticError>`, errors propagate naturally:

```rust
async fn process_query(model: &dyn ChatModel, query: &str) -> Result<String, SynapticError> {
    let messages = vec![Message::human(query)];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await?;  // Model error propagates
    Ok(response.message.content().to_string())
}
```

There is no need for `.map_err()` conversions in application code. A `Model` error from a provider adapter, a `Tool` error from execution, or a `Graph` error from the state machine all flow through the same `Result` type.

## Retry and Fallback Patterns

Not all errors are fatal. Synaptic provides several mechanisms for resilience:

### RetryChatModel

Wraps a `ChatModel` and retries on transient failures:

```rust
use synaptic::models::RetryChatModel;

let robust_model = RetryChatModel::new(model, max_retries, delay);
```

On failure, it waits and retries up to `max_retries` times. This handles transient network errors and rate limits without application code needing to implement retry logic.

### RateLimitedChatModel and TokenBucketChatModel

Proactively prevent rate limit errors by throttling requests:

- `RateLimitedChatModel` limits requests per time window.
- `TokenBucketChatModel` uses a token bucket algorithm for smooth rate limiting.

By throttling before hitting the provider's limit, these wrappers convert potential `RateLimit` errors into controlled delays.

### RunnableWithFallbacks

Tries alternative runnables when the primary one fails:

```rust
use synaptic::runnables::RunnableWithFallbacks;

let chain = RunnableWithFallbacks::new(
    primary.boxed(),
    vec![fallback_1.boxed(), fallback_2.boxed()],
);
```

If `primary` fails, `fallback_1` is tried with the same input. If that also fails, `fallback_2` is tried. Only if all options fail does the error propagate.

### RunnableRetry

Retries a runnable with configurable exponential backoff:

```rust
use std::time::Duration;
use synaptic::runnables::{RunnableRetry, RetryPolicy};

let retry = RunnableRetry::new(
    flaky_step.boxed(),
    RetryPolicy::default()
        .with_max_attempts(4)
        .with_base_delay(Duration::from_millis(200))
        .with_max_delay(Duration::from_secs(5)),
);
```

The delay doubles after each attempt (200ms, 400ms, 800ms, ...) up to `max_delay`. You can also set a `retry_on` predicate to only retry specific error types. This is useful for any step in an LCEL chain, not just model calls.

### HandleErrorTool

Wraps a tool so that errors are returned as string results instead of propagating:

```rust
use synaptic::tools::HandleErrorTool;

let safe_tool = HandleErrorTool::new(risky_tool);
```

When the inner tool fails, the error message becomes the tool's output. The LLM sees the error and can decide to retry with different arguments or take a different approach. This prevents a single tool failure from crashing the entire agent loop.

## Graph Interrupts (Not Errors)

Human-in-the-loop interrupts in the graph system are **not** errors. Graph `invoke()` returns `GraphResult<S>`, which is either `Complete(state)` or `Interrupted(state)`:

```rust
use synaptic::graph::GraphResult;

match graph.invoke(state).await? {
    GraphResult::Complete(final_state) => {
        // Graph finished normally
        handle_result(final_state);
    }
    GraphResult::Interrupted(partial_state) => {
        // Human-in-the-loop: inspect state, get approval, resume
        // The graph has checkpointed its state automatically
    }
}
```

To extract the state regardless of completion status, use `.into_state()`:

```rust
let state = graph.invoke(initial).await?.into_state();
```

Interrupts can also be triggered programmatically via `Command::interrupt()` from within a node:

```rust
use synaptic::graph::Command;

// Inside a node's process() method:
Command::interrupt(updated_state)
```

`SynapticError::Graph` is reserved for true errors: compilation failures, missing nodes, routing errors, and recursion limit violations.

## Matching on Error Variants

Since `SynapticError` is an enum, you can match on specific variants to implement targeted error handling:

```rust
match result {
    Ok(value) => use_value(value),
    Err(SynapticError::RateLimit(_)) => {
        // Wait and retry
    }
    Err(SynapticError::ToolNotFound(name)) => {
        // Log the missing tool and continue without it
    }
    Err(SynapticError::Parsing(msg)) => {
        // LLM output was malformed; ask the model to try again
    }
    Err(e) => {
        // All other errors: propagate
        return Err(e);
    }
}
```

This pattern is especially useful in agent loops where some errors are recoverable (the model can try again) and others are not (network is down, API key is invalid).

## See Also

- [Retry & Rate Limiting](../how-to/chat-models/retry-rate-limit.md) -- automatic retry for model errors
- [Fallbacks](../how-to/runnables/fallbacks.md) -- fallback chains for error recovery
- [Interrupt & Resume](../how-to/graph/interrupt-resume.md) -- graph interrupts (not errors)
