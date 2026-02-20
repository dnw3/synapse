# Middleware Macros

Synaptic provides seven macros for defining agent middleware. Each one generates:

* A struct named `{PascalCase}Middleware` (e.g. `log_response` becomes
  `LogResponseMiddleware`).
* An `impl AgentMiddleware for {PascalCase}Middleware` with the corresponding
  hook method overridden.
* A factory function with the original name that returns
  `Arc<dyn AgentMiddleware>`.

None of the middleware macros accept attribute arguments. However, all middleware
macros support `#[field]` parameters for building **stateful middleware** (see
[Stateful Middleware with `#[field]`](#stateful-middleware-with-field) below).

---

## `#[before_agent]`

Runs before the agent loop starts. The function receives a mutable reference to
the message list.

**Signature:** `async fn(messages: &mut Vec<Message>) -> Result<(), SynapticError>`

```rust,ignore
use synaptic::macros::before_agent;
use synaptic::core::{Message, SynapticError};

#[before_agent]
async fn inject_system(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    println!("Starting agent with {} messages", messages.len());
    Ok(())
}

let mw = inject_system(); // Arc<dyn AgentMiddleware>
```

## `#[before_model]`

Runs before each model call. Use this to modify the request (e.g., add headers,
tweak temperature, inject a system prompt).

**Signature:** `async fn(request: &mut ModelRequest) -> Result<(), SynapticError>`

```rust,ignore
use synaptic::macros::before_model;
use synaptic::middleware::ModelRequest;
use synaptic::core::SynapticError;

#[before_model]
async fn set_temperature(request: &mut ModelRequest) -> Result<(), SynapticError> {
    request.temperature = Some(0.7);
    Ok(())
}

let mw = set_temperature(); // Arc<dyn AgentMiddleware>
```

## `#[after_model]`

Runs after each model call. Use this to inspect or mutate the response.

**Signature:** `async fn(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError>`

```rust,ignore
use synaptic::macros::after_model;
use synaptic::middleware::{ModelRequest, ModelResponse};
use synaptic::core::SynapticError;

#[after_model]
async fn log_usage(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError> {
    if let Some(usage) = &response.usage {
        println!("Tokens used: {}", usage.total_tokens);
    }
    Ok(())
}

let mw = log_usage(); // Arc<dyn AgentMiddleware>
```

## `#[after_agent]`

Runs after the agent loop finishes. Receives the final message list.

**Signature:** `async fn(messages: &mut Vec<Message>) -> Result<(), SynapticError>`

```rust,ignore
use synaptic::macros::after_agent;
use synaptic::core::{Message, SynapticError};

#[after_agent]
async fn summarize(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    println!("Agent finished with {} messages", messages.len());
    Ok(())
}

let mw = summarize(); // Arc<dyn AgentMiddleware>
```

## `#[wrap_model_call]`

Wraps the model call with custom logic, giving you full control over whether and
how the underlying model is invoked. This is the right hook for retries,
fallbacks, caching, or circuit-breaker patterns.

**Signature:** `async fn(request: ModelRequest, next: &dyn ModelCaller) -> Result<ModelResponse, SynapticError>`

```rust,ignore
use synaptic::macros::wrap_model_call;
use synaptic::middleware::{ModelRequest, ModelResponse, ModelCaller};
use synaptic::core::SynapticError;

#[wrap_model_call]
async fn retry_once(
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    match next.call(request.clone()).await {
        Ok(response) => Ok(response),
        Err(_) => next.call(request).await, // retry once
    }
}

let mw = retry_once(); // Arc<dyn AgentMiddleware>
```

## `#[wrap_tool_call]`

Wraps individual tool calls. Same pattern as `#[wrap_model_call]` but for tool
invocations. Useful for logging, permission checks, or sandboxing.

**Signature:** `async fn(request: ToolCallRequest, next: &dyn ToolCaller) -> Result<Value, SynapticError>`

```rust,ignore
use synaptic::macros::wrap_tool_call;
use synaptic::middleware::{ToolCallRequest, ToolCaller};
use synaptic::core::SynapticError;
use serde_json::Value;

#[wrap_tool_call]
async fn log_tool(
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    println!("Calling tool: {}", request.call.name);
    let result = next.call(request).await?;
    println!("Tool returned: {}", result);
    Ok(result)
}

let mw = log_tool(); // Arc<dyn AgentMiddleware>
```

## `#[dynamic_prompt]`

Generates a system prompt dynamically based on the current conversation. Unlike
the other middleware macros, the decorated function is **synchronous** (not
async). It reads the message history and returns a `String` that is set as the
system prompt before each model call.

Under the hood, the macro generates a middleware whose `before_model` hook sets
`request.system_prompt` to the return value of your function.

**Signature:** `fn(messages: &[Message]) -> String`

```rust,ignore
use synaptic::macros::dynamic_prompt;
use synaptic::core::Message;

#[dynamic_prompt]
fn context_aware_prompt(messages: &[Message]) -> String {
    if messages.len() > 10 {
        "Be concise. The conversation is getting long.".into()
    } else {
        "Be thorough and detailed in your responses.".into()
    }
}

let mw = context_aware_prompt(); // Arc<dyn AgentMiddleware>
```

> **Why is `#[dynamic_prompt]` synchronous?**
>
> Unlike the other middleware macros, `#[dynamic_prompt]` takes a plain `fn`
> instead of `async fn`. This is a deliberate design choice:
>
> 1. **Pure computation** — Dynamic prompt generation typically involves
>    inspecting the message list and building a string. These are pure CPU
>    operations (pattern matching, string formatting) with no I/O involved.
>    Making them async would add unnecessary overhead (Future state machine,
>    poll machinery) for zero benefit.
>
> 2. **Simplicity** — Synchronous functions are easier to write and reason
>    about. No `.await`, no pinning, no Send/Sync bounds to worry about.
>
> 3. **Internal async wrapping** — The macro generates a `before_model` hook
>    that calls your sync function inside an async context. The hook itself
>    is async (as required by `AgentMiddleware`), but your function doesn't
>    need to be.
>
> If you need async operations in your prompt generation (e.g., fetching
> context from a database or calling an API), use `#[before_model]` directly
> and set `request.system_prompt` yourself:
>
> ```rust,ignore
> #[before_model]
> async fn async_prompt(request: &mut ModelRequest) -> Result<(), SynapticError> {
>     let context = fetch_from_database().await?;  // async I/O
>     request.system_prompt = Some(format!("Context: {}", context));
>     Ok(())
> }
> ```

---

## Stateful Middleware with `#[field]`

All middleware macros support `#[field]` parameters — function parameters that
become struct fields rather than trait method parameters. This lets you build
middleware with configuration state, just like `#[tool]` tools with `#[field]`.

Field parameters must come **before** the trait-mandated parameters. The factory
function will accept the field values, and the generated struct stores them.

**Example: Retry middleware with configurable retries**

```rust,ignore
use std::time::Duration;
use synaptic::macros::wrap_tool_call;
use synaptic::middleware::{ToolCallRequest, ToolCaller};
use synaptic::core::SynapticError;
use serde_json::Value;

#[wrap_tool_call]
async fn tool_retry(
    #[field] max_retries: usize,
    #[field] base_delay: Duration,
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match next.call(request.clone()).await {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                if attempt < max_retries {
                    let delay = base_delay * 2u32.saturating_pow(attempt as u32);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

// Factory function accepts the field values:
let mw = tool_retry(3, Duration::from_millis(100));
```

**Example: Model fallback with alternative models**

```rust,ignore
use std::sync::Arc;
use synaptic::macros::wrap_model_call;
use synaptic::middleware::{BaseChatModelCaller, ModelRequest, ModelResponse, ModelCaller};
use synaptic::core::{ChatModel, SynapticError};

#[wrap_model_call]
async fn model_fallback(
    #[field] fallbacks: Vec<Arc<dyn ChatModel>>,
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    match next.call(request.clone()).await {
        Ok(resp) => Ok(resp),
        Err(primary_err) => {
            for fallback in &fallbacks {
                let caller = BaseChatModelCaller::new(fallback.clone());
                if let Ok(resp) = caller.call(request.clone()).await {
                    return Ok(resp);
                }
            }
            Err(primary_err)
        }
    }
}

let mw = model_fallback(vec![backup_model]);
```

**Example: Dynamic prompt with branding**

```rust,ignore
use synaptic::macros::dynamic_prompt;
use synaptic::core::Message;

#[dynamic_prompt]
fn branded_prompt(#[field] brand: String, messages: &[Message]) -> String {
    format!("[{}] You have {} messages", brand, messages.len())
}

let mw = branded_prompt("Acme Corp".into());
```
