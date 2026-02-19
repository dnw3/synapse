# ModelCallLimitMiddleware

Limits the number of model invocations during a single agent run, preventing runaway loops. Use this when you want a hard cap on how many times the LLM is called per invocation.

## Constructor

```rust,ignore
use synaptic_middleware::ModelCallLimitMiddleware;

let mw = ModelCallLimitMiddleware::new(10); // max 10 model calls
```

The middleware also exposes `call_count()` to inspect the current count and `reset()` to zero it out.

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ModelCallLimitMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(5)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `wrap_model_call`
- Before delegating to the next layer, the middleware atomically increments an internal counter.
- If the counter has reached or exceeded `max_calls`, it returns `SynapticError::MaxStepsExceeded` immediately without calling the model.
- Otherwise, it delegates to `next.call(request)` as normal.

This means the agent loop terminates with an error once the limit is hit. The counter persists across the entire agent invocation (all steps in the agent loop), so a limit of 5 means at most 5 model round-trips total.

## Example: Combining with Other Middleware

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(10)),
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

The model call limit is checked on every model call regardless of whether other middlewares modify the request or response.
