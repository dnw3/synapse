# ToolRetryMiddleware

Retries failed tool calls with exponential backoff. Use this when tools may experience transient failures (network timeouts, rate limits, temporary unavailability) and you want automatic recovery without surfacing errors to the model.

## Constructor

```rust,ignore
use synaptic_middleware::ToolRetryMiddleware;

// Retry up to 3 times (4 total attempts including the first)
let mw = ToolRetryMiddleware::new(3);
```

### Configuration

The base delay between retries defaults to 100ms and doubles on each attempt (exponential backoff). You can customize it with `with_base_delay`:

```rust,ignore
use std::time::Duration;

let mw = ToolRetryMiddleware::new(3)
    .with_base_delay(Duration::from_millis(500));
// Delays: 500ms, 1000ms, 2000ms
```

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ToolRetryMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `wrap_tool_call`
- When a tool call fails, the middleware waits for `base_delay * 2^attempt` and retries.
- Retries continue up to `max_retries` times. If all retries fail, the last error is returned.
- If the tool call succeeds on any attempt, the result is returned immediately.

The backoff schedule with the default 100ms base delay:

| Attempt | Delay before retry |
|---------|--------------------|
| 1st retry | 100ms |
| 2nd retry | 200ms |
| 3rd retry | 400ms |

## Combining with Tool Call Limits

When both middlewares are active, the retry middleware operates inside the tool call limit. Each retry counts as a separate tool call:

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolCallLimitMiddleware::new(30)),
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```
