# ToolCallLimitMiddleware

Limits the number of tool invocations during a single agent run. Use this to cap tool usage when agents may generate excessive tool calls in a loop.

## Constructor

```rust,ignore
use synaptic_middleware::ToolCallLimitMiddleware;

let mw = ToolCallLimitMiddleware::new(20); // max 20 tool calls
```

The middleware exposes `call_count()` and `reset()` for inspection and manual reset.

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ToolCallLimitMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolCallLimitMiddleware::new(20)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `wrap_tool_call`
- Each time a tool call is dispatched, the middleware atomically increments an internal counter.
- If the counter has reached or exceeded `max_calls`, it returns `SynapticError::MaxStepsExceeded` without executing the tool.
- Otherwise, it delegates to `next.call(request)` normally.

The counter tracks individual tool calls, not agent steps. If a single model response requests three tool calls, the counter increments three times. This gives you precise control over total tool usage across the entire agent run.

## Combining Model and Tool Limits

Both limits can be applied simultaneously to guard against different failure modes:

```rust,ignore
use synaptic_middleware::{ModelCallLimitMiddleware, ToolCallLimitMiddleware};

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(10)),
        Arc::new(ToolCallLimitMiddleware::new(30)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

The agent stops as soon as either limit is hit.

## Handling the Error

When the limit is exceeded, the middleware returns `SynapticError::MaxStepsExceeded`. You can catch this to provide a graceful fallback:

```rust,ignore
use synaptic_core::SynapticError;

let mut state = MessageState::new();
state.messages.push(Message::human("Do something complex."));

match graph.invoke(state).await {
    Ok(result) => println!("{}", result.into_state().messages.last().unwrap().content()),
    Err(SynapticError::MaxStepsExceeded(msg)) => {
        println!("Agent hit tool call limit: {msg}");
        // Retry with a higher limit, summarize progress, or inform the user
    }
    Err(e) => println!("Other error: {e}"),
}
```

## Inspecting and Resetting

The middleware provides methods to inspect and reset the counter:

```rust,ignore
let mw = ToolCallLimitMiddleware::new(10);

// After an agent run, check how many tool calls were made
println!("Tool calls used: {}", mw.call_count());

// Reset the counter for a new run
mw.reset();
assert_eq!(mw.call_count(), 0);
```
