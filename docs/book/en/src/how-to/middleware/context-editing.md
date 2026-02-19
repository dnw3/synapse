# ContextEditingMiddleware

Trims or filters the conversation context before each model call. Use this to keep the context window manageable when full summarization is unnecessary -- for example, dropping old messages or stripping tool call noise from the history.

## Constructor

The middleware accepts a `ContextStrategy` that defines how messages are edited:

```rust,ignore
use synaptic_middleware::{ContextEditingMiddleware, ContextStrategy};

// Keep only the last 10 non-system messages
let mw = ContextEditingMiddleware::new(ContextStrategy::LastN(10));

// Remove tool call/result pairs, keeping only human/AI content messages
let mw = ContextEditingMiddleware::new(ContextStrategy::StripToolCalls);

// Strip tool calls first, then keep last N
let mw = ContextEditingMiddleware::new(ContextStrategy::StripAndTruncate(10));
```

### Convenience Constructors

```rust,ignore
let mw = ContextEditingMiddleware::last_n(10);
let mw = ContextEditingMiddleware::strip_tool_calls();
```

## Strategies

| Strategy | Behavior |
|----------|----------|
| `LastN(n)` | Keeps leading system messages, then the last `n` non-system messages |
| `StripToolCalls` | Removes `Tool` messages and AI messages that contain only tool calls (no text) |
| `StripAndTruncate(n)` | Applies `StripToolCalls` first, then `LastN(n)` |

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ContextEditingMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ContextEditingMiddleware::last_n(20)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `before_model`
- Before each model call, the middleware applies the configured strategy to `request.messages`.
- **LastN:** System messages at the start of the list are always preserved. From the remaining messages, only the last `n` are kept. Earlier messages are dropped.
- **StripToolCalls:** Messages with `is_tool() == true` are removed. AI messages that have tool calls but empty text content are also removed. This cleans up the tool-call/tool-result pairs while preserving the conversational content.
- **StripAndTruncate:** Runs both filters in sequence -- first strips tool calls, then truncates to the last N.

The original message list in the agent state is not modified; only the request sent to the model is trimmed.

## Example: Combining with Summarization

For maximum context efficiency, strip tool calls first, then summarize what remains:

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ContextEditingMiddleware::strip_tool_calls()),
        Arc::new(SummarizationMiddleware::new(model.clone(), 4000, |msg| msg.content().len() / 4)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

The context editor removes tool noise before summarization runs, producing cleaner summaries.
