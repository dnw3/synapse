# Procedural Macros

The `synaptic-macros` crate ships 12 attribute macros that eliminate boilerplate
when building agents with Synaptic. Instead of manually implementing traits such
as `Tool`, `AgentMiddleware`, or `Entrypoint`, you annotate an ordinary function
and the macro generates the struct, the trait implementation, and a factory
function for you.

All macros live in the `synaptic_macros` crate and are re-exported through the
`synaptic` facade, so you can import them with:

```rust,ignore
use synaptic::macros::*;       // all macros at once
use synaptic::macros::tool;    // or pick individually
```

---

## `#[tool]` -- Define Tools from Functions

`#[tool]` converts an `async fn` into a full `Tool` (or `RuntimeAwareTool`)
implementation. The macro generates:

* A struct named `{PascalCase}Tool` (e.g. `web_search` becomes `WebSearchTool`).
* An `impl Tool for WebSearchTool` block with `name()`, `description()`,
  `parameters()` (JSON Schema), and `call()`.
* A factory function with the original name that returns `Arc<dyn Tool>`.

### Basic Usage

```rust,ignore
use synaptic::macros::tool;
use synaptic::core::SynapticError;

/// Search the web for a given query.
#[tool]
async fn web_search(query: String) -> Result<String, SynapticError> {
    Ok(format!("Results for '{}'", query))
}

// The macro produces:
//   struct WebSearchTool;
//   impl Tool for WebSearchTool { ... }
//   fn web_search() -> Arc<dyn Tool> { ... }

let tool = web_search();
assert_eq!(tool.name(), "web_search");
```

### Doc Comments as Description

The doc comment on the function becomes the tool description that is sent to the
LLM. Write a clear, concise sentence -- this is what the model reads when
deciding whether to call your tool.

```rust,ignore
/// Fetch the current weather for a city.
#[tool]
async fn get_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("Sunny in {}", city))
}

let tool = get_weather();
assert_eq!(tool.description(), "Fetch the current weather for a city.");
```

You can also override the description explicitly:

```rust,ignore
#[tool(description = "Look up weather information.")]
async fn get_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("Sunny in {}", city))
}
```

### Parameter Types and JSON Schema

Each function parameter is mapped to a JSON Schema property automatically.
The following type mappings are supported:

| Rust Type | JSON Schema |
|-----------|-------------|
| `String`  | `{"type": "string"}` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `isize` | `{"type": "integer"}` |
| `f32`, `f64` | `{"type": "number"}` |
| `bool` | `{"type": "boolean"}` |
| `Vec<T>` | `{"type": "array", "items": <schema of T>}` |
| `serde_json::Value` | `{"type": "object"}` |
| `T: JsonSchema` (with `schemars` feature) | Full schema from schemars |
| Any other type (without `schemars`) | `{"type": "object"}` (fallback) |

Parameter doc comments become `"description"` in the JSON Schema, giving the LLM
extra context about what to pass:

```rust,ignore
#[tool]
async fn search(
    /// The search query string
    query: String,
    /// Maximum number of results to return
    max_results: i64,
) -> Result<String, SynapticError> {
    Ok(format!("Searching '{}' (limit {})", query, max_results))
}
```

This generates a JSON Schema similar to:

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "The search query string" },
    "max_results": { "type": "integer", "description": "Maximum number of results to return" }
  },
  "required": ["query", "max_results"]
}
```

### Custom Types with `schemars`

By default, custom struct parameters generate a minimal `{"type": "object"}` schema
with no field details — the LLM has no guidance about the struct's shape. To generate
full schemas for custom types, enable the `schemars` feature and derive `JsonSchema`
on your parameter types.

**Enable the feature** in your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["macros", "schemars"] }
schemars = { version = "0.8", features = ["derive"] }
```

**Derive `JsonSchema`** on your parameter types:

```rust,ignore
use schemars::JsonSchema;
use serde::Deserialize;
use synaptic::macros::tool;
use synaptic::core::SynapticError;

#[derive(Deserialize, JsonSchema)]
struct UserInfo {
    /// User's display name
    name: String,
    /// Age in years
    age: i32,
    email: Option<String>,
}

/// Process user information.
#[tool]
async fn process_user(
    /// The user to process
    user: UserInfo,
    /// Action to perform
    action: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}: {}", user.name, action))
}
```

**Without schemars**, `user` generates:

```json
{ "type": "object", "description": "The user to process" }
```

**With schemars**, `user` generates a full schema:

```json
{
  "type": "object",
  "description": "The user to process",
  "properties": {
    "name": { "type": "string" },
    "age": { "type": "integer", "format": "int32" },
    "email": { "type": "string" }
  },
  "required": ["name", "age"]
}
```

Nested types work automatically — if `UserInfo` contained an `Address` struct that
also derives `JsonSchema`, the address schema is included via `$defs` references.

> **Note:** Known primitive types (`String`, `i32`, `Vec<T>`, `bool`, etc.) always
> use the built-in hardcoded schemas regardless of whether `schemars` is enabled.
> Only unknown/custom types benefit from the `schemars` integration.

### Optional Parameters (`Option<T>`)

Wrap a parameter in `Option<T>` to make it optional. Optional parameters are
excluded from the `"required"` array in the schema. At runtime, missing or
`null` JSON values are deserialized as `None`.

```rust,ignore
#[tool]
async fn search(
    query: String,
    /// Filter by language (optional)
    language: Option<String>,
) -> Result<String, SynapticError> {
    let lang = language.unwrap_or_else(|| "en".into());
    Ok(format!("Searching '{}' in {}", query, lang))
}
```

### Default Values (`#[default = ...]`)

Use `#[default = value]` on a parameter to supply a compile-time default.
Parameters with defaults are not required in the schema, and the default is
recorded in the `"default"` field of the schema property.

```rust,ignore
#[tool]
async fn search(
    query: String,
    #[default = 10]
    max_results: i64,
    #[default = "en"]
    language: String,
) -> Result<String, SynapticError> {
    Ok(format!("Searching '{}' (max {}, lang {})", query, max_results, language))
}
```

If the LLM omits `max_results`, it defaults to `10`. If it omits `language`,
it defaults to `"en"`.

### Custom Tool Name (`#[tool(name = "...")]`)

By default the tool name matches the function name. Override it with the `name`
attribute when you need a different identifier exposed to the LLM:

```rust,ignore
#[tool(name = "google_search")]
async fn search(query: String) -> Result<String, SynapticError> {
    Ok(format!("Searching for '{}'", query))
}

let tool = search();
assert_eq!(tool.name(), "google_search");
```

The factory function keeps the original Rust name (`search()`), but
`tool.name()` returns `"google_search"`.

### Struct Fields (`#[field]`)

Some tools need to hold state — a database connection, an API client, a backend
reference, etc. Mark those parameters with `#[field]` and they become struct
fields instead of JSON Schema parameters. The factory function will require
these values at construction time, and they are hidden from the LLM entirely.

```rust,ignore
use std::sync::Arc;
use synaptic::core::SynapticError;
use serde_json::Value;

#[tool]
async fn db_lookup(
    #[field] connection: Arc<String>,
    /// The table to query
    table: String,
) -> Result<String, SynapticError> {
    Ok(format!("Querying {} on {}", table, connection))
}

// Factory now requires the field parameter:
let tool = db_lookup(Arc::new("postgres://localhost".into()));
assert_eq!(tool.name(), "db_lookup");
// Only "table" appears in the schema; "connection" is hidden
```

The macro generates a struct with the field:

```rust,ignore
struct DbLookupTool {
    connection: Arc<String>,
}
```

You can combine `#[field]` with regular parameters, `Option<T>`, and
`#[default = ...]`. Multiple `#[field]` parameters are supported:

```rust,ignore
#[tool]
async fn annotate(
    #[field] prefix: String,
    #[field] suffix: String,
    /// The input text
    text: String,
    #[default = 1]
    repeat: i64,
) -> Result<String, SynapticError> {
    let inner = text.repeat(repeat as usize);
    Ok(format!("{}{}{}", prefix, inner, suffix))
}

let tool = annotate("<<".into(), ">>".into());
```

> **Note:** `#[field]` and `#[inject]` cannot be used on the same parameter.
> Use `#[field]` when the value is provided at construction time; use
> `#[inject]` when it comes from the agent runtime.

### Raw Arguments (`#[args]`)

Some tools need to receive the raw JSON arguments without any deserialization —
for example, echo tools that forward the entire input, or tools that handle
arbitrary JSON payloads. Mark the parameter with `#[args]` and it will receive
the raw `serde_json::Value` passed to `call()` directly.

```rust,ignore
use synaptic::macros::tool;
use synaptic::core::SynapticError;
use serde_json::{json, Value};

/// Echo the input back.
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(json!({"echo": args}))
}

let tool = echo();
assert_eq!(tool.name(), "echo");

// parameters() returns None — no JSON Schema is generated
assert!(tool.parameters().is_none());
```

The `#[args]` parameter:

- Receives the raw `Value` without any JSON Schema generation or deserialization
- Causes `parameters()` to return `None` (unless there are other normal parameters)
- Can be combined with `#[field]` parameters (struct fields are still supported)
- Cannot be combined with `#[inject]` on the same parameter
- At most one parameter can be marked `#[args]`

```rust,ignore
/// Echo with a configurable prefix.
#[tool]
async fn echo_with_prefix(
    #[field] prefix: String,
    #[args] args: Value,
) -> Result<Value, SynapticError> {
    Ok(json!({"prefix": prefix, "data": args}))
}

let tool = echo_with_prefix(">>".into());
```

### Runtime Injection (`#[inject(state)]`, `#[inject(store)]`, `#[inject(tool_call_id)]`)

Some tools need access to agent runtime state that the LLM should not (and
cannot) provide. Mark those parameters with `#[inject(...)]` and they will be
populated from the `ToolRuntime` context instead of from the LLM-supplied JSON
arguments. Injected parameters are hidden from the JSON Schema entirely.

When any parameter uses `#[inject(...)]`, the macro generates a
`RuntimeAwareTool` implementation (with `call_with_runtime`) instead of a plain
`Tool`.

There are three injection kinds:

| Annotation | Source | Typical Type |
|------------|--------|-------------|
| `#[inject(state)]` | `ToolRuntime::state` (deserialized from `Value`) | Your state struct, or `Value` |
| `#[inject(store)]` | `ToolRuntime::store` (cloned `Option<Arc<dyn Store>>`) | `Arc<dyn Store>` |
| `#[inject(tool_call_id)]` | `ToolRuntime::tool_call_id` (the ID of the current call) | `String` |

```rust,ignore
use synaptic::core::{SynapticError, ToolRuntime};
use std::sync::Arc;

#[tool]
async fn save_note(
    /// The note content
    content: String,
    /// Injected: the current tool call ID
    #[inject(tool_call_id)]
    call_id: String,
    /// Injected: shared application state
    #[inject(state)]
    state: serde_json::Value,
) -> Result<String, SynapticError> {
    Ok(format!("Saved note (call={}) with state {:?}", call_id, state))
}

// Factory returns Arc<dyn RuntimeAwareTool> instead of Arc<dyn Tool>
let tool = save_note();
```

The LLM only sees `content` in the schema; `call_id` and `state` are supplied
by the agent runtime automatically.

---

## `#[chain]` -- Create Runnable Chains

`#[chain]` wraps an `async fn` as a `BoxRunnable`. It is a lightweight way to
create composable runnable steps that can be piped together.

The macro generates:

* A private `{name}_impl` function containing the original body.
* A public factory function with the original name that returns a
  `BoxRunnable<InputType, OutputType>` backed by a `RunnableLambda`.

### Output Type Inference

The macro automatically detects the return type:

| Return Type | Generated Type | Behavior |
|-------------|---------------|----------|
| `Result<Value, _>` | `BoxRunnable<I, Value>` | Serializes result to `Value` |
| `Result<String, _>` | `BoxRunnable<I, String>` | Returns directly, no serialization |
| `Result<T, _>` (any other) | `BoxRunnable<I, T>` | Returns directly, no serialization |

### Basic Usage

```rust,ignore
use synaptic::macros::chain;
use synaptic::core::SynapticError;
use serde_json::Value;

// Value output — result is serialized to Value
#[chain]
async fn uppercase(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default().to_uppercase();
    Ok(Value::String(s))
}

// `uppercase()` returns BoxRunnable<Value, Value>
let runnable = uppercase();
```

### Typed Output

When the return type is not `Value`, the macro generates a typed runnable
without serialization overhead:

```rust,ignore
// String output — returns BoxRunnable<String, String>
#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[chain]
async fn exclaim(s: String) -> Result<String, SynapticError> {
    Ok(format!("{}!", s))
}

// Typed chains compose naturally with |
let pipeline = to_upper() | exclaim();
let result = pipeline.invoke("hello".into(), &config).await?;
assert_eq!(result, "HELLO!");
```

### Composition with `|`

Runnables support pipe-based composition. Chain multiple steps together by
combining the factories:

```rust,ignore
#[chain]
async fn step_a(input: Value) -> Result<Value, SynapticError> {
    // ... transform input ...
    Ok(input)
}

#[chain]
async fn step_b(input: Value) -> Result<Value, SynapticError> {
    // ... transform further ...
    Ok(input)
}

// Compose into a pipeline: step_a | step_b
let pipeline = step_a() | step_b();
let result = pipeline.invoke(serde_json::json!("hello")).await?;
```

> **Note:** `#[chain]` does not accept any arguments. Attempting to write
> `#[chain(name = "...")]` will produce a compile error.

---

## `#[entrypoint]` -- Workflow Entry Points

`#[entrypoint]` defines a LangGraph-style workflow entry point. The macro
generates a factory function that returns a `synaptic::core::Entrypoint` struct
containing the configuration and a boxed async closure.

The decorated function must:

* Be `async`.
* Accept exactly one parameter of type `serde_json::Value`.
* Return `Result<Value, SynapticError>`.

### Basic Usage

```rust,ignore
use synaptic::macros::entrypoint;
use synaptic::core::SynapticError;
use serde_json::Value;

#[entrypoint]
async fn my_workflow(input: Value) -> Result<Value, SynapticError> {
    // orchestrate agents, tools, subgraphs...
    Ok(input)
}

let ep = my_workflow();
// ep.config.name == "my_workflow"
```

### Attributes (`name`, `checkpointer`)

| Attribute | Default | Description |
|-----------|---------|-------------|
| `name = "..."` | function name | Override the entrypoint name |
| `checkpointer = "..."` | `None` | Hint which checkpointer backend to use (e.g. `"memory"`, `"redis"`) |

```rust,ignore
#[entrypoint(name = "chat_bot", checkpointer = "memory")]
async fn my_workflow(input: Value) -> Result<Value, SynapticError> {
    Ok(input)
}

let ep = my_workflow();
assert_eq!(ep.config.name, "chat_bot");
assert_eq!(ep.config.checkpointer, Some("memory"));
```

---

## `#[task]` -- Trackable Tasks

`#[task]` marks an async function as a named task. This is useful inside
entrypoints for tracing and streaming identification. The macro:

* Renames the original function to `{name}_impl`.
* Creates a public wrapper function that defines a `__TASK_NAME` constant and
  delegates to the impl.

### Basic Usage

```rust,ignore
use synaptic::macros::task;
use synaptic::core::SynapticError;

#[task]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("Sunny in {}", city))
}

// Calling fetch_weather("Paris".into()) internally sets __TASK_NAME = "fetch_weather"
// and delegates to fetch_weather_impl("Paris".into()).
let result = fetch_weather("Paris".into()).await?;
```

### Custom Task Name

Override the task name with `name = "..."`:

```rust,ignore
#[task(name = "weather_lookup")]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("Sunny in {}", city))
}
// __TASK_NAME is now "weather_lookup"
```

---

## Middleware Macros

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

### `#[before_agent]`

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

### `#[before_model]`

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

### `#[after_model]`

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

### `#[after_agent]`

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

### `#[wrap_model_call]`

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

### `#[wrap_tool_call]`

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

### `#[dynamic_prompt]`

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

### Stateful Middleware with `#[field]`

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

---

## `#[traceable]` -- Tracing Instrumentation

`#[traceable]` adds `tracing` instrumentation to any function. It wraps the
function body in a `tracing::info_span!` with parameter values recorded as span
fields. For async functions, the span is propagated correctly using
`tracing::Instrument`.

### Basic Usage

```rust,ignore
use synaptic::macros::traceable;

#[traceable]
async fn process_data(input: String, count: usize) -> String {
    format!("{}: {}", input, count)
}
```

This generates code equivalent to:

```rust,ignore
async fn process_data(input: String, count: usize) -> String {
    use tracing::Instrument;
    let __span = tracing::info_span!(
        "process_data",
        input = tracing::field::debug(&input),
        count = tracing::field::debug(&count),
    );
    async move {
        format!("{}: {}", input, count)
    }
    .instrument(__span)
    .await
}
```

For synchronous functions, the macro uses a span guard instead of `Instrument`:

```rust,ignore
#[traceable]
fn compute(x: i32, y: i32) -> i32 {
    x + y
}
// Generates a span guard: let __enter = __span.enter();
```

### Custom Span Name

Override the default span name (which is the function name) with `name = "..."`:

```rust,ignore
#[traceable(name = "data_pipeline")]
async fn process_data(input: String) -> String {
    input.to_uppercase()
}
// The span is named "data_pipeline" instead of "process_data"
```

### Skipping Parameters

Exclude sensitive or large parameters from being recorded in the span with
`skip = "param1,param2"`:

```rust,ignore
#[traceable(skip = "api_key")]
async fn call_api(query: String, api_key: String) -> Result<String, SynapticError> {
    // `query` is recorded in the span, `api_key` is not
    Ok(format!("Called API with '{}'", query))
}
```

You can combine both attributes:

```rust,ignore
#[traceable(name = "api_call", skip = "api_key,secret")]
async fn call_api(query: String, api_key: String, secret: String) -> Result<String, SynapticError> {
    Ok("done".into())
}
```

---

## Complete Examples

The following end-to-end scenarios show how the macros work together in
realistic applications.

### Scenario A: Weather Agent with Custom Tool

This example defines a tool with `#[tool]` and a `#[field]` for an API key,
registers it, creates a ReAct agent with `create_react_agent`, and runs a
query.

```rust,ignore
use synaptic::core::{ChatModel, Message, SynapticError};
use synaptic::graph::{create_react_agent, MessageState, GraphResult};
use synaptic::models::ScriptedChatModel;
use std::sync::Arc;

/// Get the current weather for a city.
#[tool]
async fn get_weather(
    #[field] api_key: String,
    /// City name to look up
    city: String,
) -> Result<String, SynapticError> {
    // In production, call a real weather API with api_key
    Ok(format!("72°F and sunny in {}", city))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let tool = get_weather("sk-fake-key".into());
    let tools: Vec<Arc<dyn synaptic::core::Tool>> = vec![tool];

    let model: Arc<dyn ChatModel> = Arc::new(ScriptedChatModel::new(vec![/* ... */]));
    let agent = create_react_agent(model, tools).compile()?;

    let state = MessageState::from_messages(vec![
        Message::human("What's the weather in Tokyo?"),
    ]);

    let result = agent.invoke(state, None).await?;
    println!("{:?}", result.into_state().messages);
    Ok(())
}
```

### Scenario B: Data Pipeline with Chain Macros

This example composes multiple `#[chain]` steps into a processing pipeline
that extracts text, normalizes it, and counts words.

```rust,ignore
use synaptic::core::{RunnableConfig, SynapticError};
use synaptic::runnables::Runnable;
use serde_json::{json, Value};

#[chain]
async fn extract_text(input: Value) -> Result<Value, SynapticError> {
    let text = input["content"].as_str().unwrap_or("");
    Ok(json!(text.to_string()))
}

#[chain]
async fn normalize(input: Value) -> Result<Value, SynapticError> {
    let text = input.as_str().unwrap_or("").to_lowercase().trim().to_string();
    Ok(json!(text))
}

#[chain]
async fn word_count(input: Value) -> Result<Value, SynapticError> {
    let text = input.as_str().unwrap_or("");
    let count = text.split_whitespace().count();
    Ok(json!({"text": text, "word_count": count}))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let pipeline = extract_text() | normalize() | word_count();
    let config = RunnableConfig::default();

    let input = json!({"content": "  Hello World  from Synaptic!  "});
    let result = pipeline.invoke(input, &config).await?;

    println!("Result: {}", result);
    // {"text": "hello world from synaptic!", "word_count": 4}
    Ok(())
}
```

### Scenario C: Agent with Middleware Stack

This example combines middleware macros into a real agent with logging, retry,
and dynamic prompting.

```rust,ignore
use synaptic::core::{Message, SynapticError};
use synaptic::middleware::{AgentMiddleware, MiddlewareChain, ModelRequest, ModelResponse, ModelCaller};
use std::sync::Arc;

// Log every model call
#[after_model]
async fn log_response(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError> {
    println!("[LOG] Model responded with {} chars",
        response.message.content().len());
    Ok(())
}

// Retry failed model calls up to 2 times
#[wrap_model_call]
async fn retry_model(
    #[field] max_retries: usize,
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    let mut last_err = None;
    for _ in 0..=max_retries {
        match next.call(request.clone()).await {
            Ok(resp) => return Ok(resp),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap())
}

// Dynamic system prompt based on conversation length
#[dynamic_prompt]
fn adaptive_prompt(messages: &[Message]) -> String {
    if messages.len() > 20 {
        "Be concise. Summarize rather than elaborate.".into()
    } else {
        "You are a helpful assistant. Be thorough.".into()
    }
}

fn build_middleware_stack() -> Vec<Arc<dyn AgentMiddleware>> {
    vec![
        adaptive_prompt(),
        retry_model(2),
        log_response(),
    ]
}
```

### Scenario D: Store-Backed Note Manager with Typed Input

This example combines `#[inject]` for runtime access and `schemars` for rich
JSON Schema generation. A `save_note` tool accepts a custom `NoteInput` struct
whose full schema (title, content, tags) is visible to the LLM, while the
shared store and tool call ID are injected transparently by the agent runtime.

**Cargo.toml** -- enable the `agent`, `store`, and `schemars` features:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["agent", "store", "schemars"] }
schemars = { version = "0.8", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**Full example:**

```rust,ignore
use std::sync::Arc;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use synaptic::core::{Store, SynapticError};
use synaptic::macros::tool;

// --- Custom input type with schemars ---
// Deriving JsonSchema gives the LLM a complete description of every field,
// including the nested Vec<String> for tags.

#[derive(Deserialize, JsonSchema)]
struct NoteInput {
    /// Title of the note
    title: String,
    /// Body content of the note (Markdown supported)
    content: String,
    /// Tags for categorisation (e.g. ["work", "urgent"])
    tags: Vec<String>,
}

// --- What the LLM sees (with schemars enabled) ---
//
// The generated JSON Schema for the `note` parameter looks like:
//
// {
//   "type": "object",
//   "properties": {
//     "title":   { "type": "string", "description": "Title of the note" },
//     "content": { "type": "string", "description": "Body content of the note (Markdown supported)" },
//     "tags":    { "type": "array",  "items": { "type": "string" },
//                  "description": "Tags for categorisation (e.g. [\"work\", \"urgent\"])" }
//   },
//   "required": ["title", "content", "tags"]
// }
//
// --- Without schemars, the same parameter would produce only: ---
//
// { "type": "object" }
//
// ...giving the LLM no guidance about the expected fields.

/// Save a note to the shared store.
#[tool]
async fn save_note(
    /// The note to save (title, content, and tags)
    note: NoteInput,
    /// Injected: persistent key-value store
    #[inject(store)]
    store: Arc<dyn Store>,
    /// Injected: the current tool call ID for traceability
    #[inject(tool_call_id)]
    call_id: String,
) -> Result<String, SynapticError> {
    // Build a unique key from the tool call ID
    let key = format!("note:{}", call_id);

    // Persist the note as a JSON item in the store
    let value = json!({
        "title":   note.title,
        "content": note.content,
        "tags":    note.tags,
        "call_id": call_id,
    });

    store.put("notes", &key, value.clone()).await?;

    Ok(format!(
        "Saved note '{}' with {} tag(s) [key={}]",
        note.title,
        note.tags.len(),
        key,
    ))
}

// Usage:
//   let tool = save_note();          // Arc<dyn RuntimeAwareTool>
//   assert_eq!(tool.name(), "save_note");
//
// The LLM sees only the `note` parameter in the schema.
// `store` and `call_id` are injected by ToolNode at runtime.
```

**Key takeaways:**

- `NoteInput` derives both `Deserialize` (for runtime deserialization) and
  `JsonSchema` (for compile-time schema generation). The `schemars` feature
  must be enabled in `Cargo.toml` for the `#[tool]` macro to pick up the
  derived schema.
- `#[inject(store)]` gives the tool direct access to the shared `Store`
  without exposing it to the LLM. The `ToolNode` populates the store from
  `ToolRuntime` before each call.
- `#[inject(tool_call_id)]` provides a unique identifier for the current
  invocation, useful for creating deterministic storage keys or audit trails.
- Because `#[inject]` is present, the macro generates a `RuntimeAwareTool`
  (not a plain `Tool`). The factory function returns
  `Arc<dyn RuntimeAwareTool>`.

### Scenario E: Workflow with Entrypoint, Tasks, and Tracing

This scenario demonstrates `#[entrypoint]`, `#[task]`, and `#[traceable]`
working together to build an instrumented data pipeline.

```rust,ignore
use synaptic::core::SynapticError;
use synaptic::macros::{entrypoint, task, traceable};
use serde_json::{json, Value};

// A helper that calls an external API. The #[traceable] macro wraps it
// in a tracing span. We skip the api_key so it never appears in logs.
#[traceable(name = "external_api_call", skip = "api_key")]
async fn call_external_api(
    url: String,
    api_key: String,
) -> Result<Value, SynapticError> {
    // In production: reqwest::get(...).await
    Ok(json!({"status": "ok", "data": [1, 2, 3]}))
}

// Each #[task] gets a stable name used by streaming and tracing.
#[task(name = "fetch")]
async fn fetch_data(source: String) -> Result<Value, SynapticError> {
    let api_key = std::env::var("API_KEY").unwrap_or_default();
    let result = call_external_api(source, api_key).await?;
    Ok(result)
}

#[task(name = "transform")]
async fn transform_data(raw: Value) -> Result<Value, SynapticError> {
    let items = raw["data"].as_array().cloned().unwrap_or_default();
    let doubled: Vec<Value> = items
        .iter()
        .filter_map(|v| v.as_i64())
        .map(|n| json!(n * 2))
        .collect();
    Ok(json!({"transformed": doubled}))
}

// The entrypoint ties the workflow together with a name and checkpointer.
#[entrypoint(name = "data_pipeline", checkpointer = "memory")]
async fn run_pipeline(input: Value) -> Result<Value, SynapticError> {
    let source = input["source"].as_str().unwrap_or("default").to_string();

    let raw = fetch_data(source).await?;
    let result = transform_data(raw).await?;

    Ok(result)
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // Set up tracing to see the spans emitted by #[traceable] and #[task]:
    //   tracing_subscriber::fmt()
    //       .with_max_level(tracing::Level::INFO)
    //       .init();

    let ep = run_pipeline();
    let output = (ep.run)(json!({"source": "https://api.example.com/data"})).await?;
    println!("Pipeline output: {}", output);
    Ok(())
}
```

**Key takeaways:**

- `#[task]` gives each step a stable name (`"fetch"`, `"transform"`) that
  appears in streaming events and tracing spans, making it easy to identify
  which step is running or failed.
- `#[traceable]` instruments any function with an automatic tracing span.
  Use `skip = "api_key"` to keep secrets out of your traces.
- `#[entrypoint]` ties the workflow together with a logical name and an
  optional `checkpointer` hint for state persistence.
- These macros are composable -- use them in any combination. A `#[task]`
  can call a `#[traceable]` helper, and an `#[entrypoint]` can orchestrate
  any number of `#[task]` functions.

### Scenario F: Tool Permission Gating with Audit Logging

This scenario demonstrates `#[wrap_tool_call]` with an allowlist field for
permission gating, plus `#[before_agent]` and `#[after_agent]` for lifecycle
audit logging.

```rust,ignore
use std::sync::Arc;
use synaptic::core::{Message, SynapticError};
use synaptic::macros::{before_agent, after_agent, wrap_tool_call};
use synaptic::middleware::{AgentMiddleware, ToolCallRequest, ToolCaller};
use serde_json::Value;

// --- Permission gating ---
// Only allow tools whose names appear in the allowlist.
// If the LLM tries to call a tool not in the list, return an error.

#[wrap_tool_call]
async fn permission_gate(
    #[field] allowed_tools: Vec<String>,
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    if !allowed_tools.contains(&request.call.name) {
        return Err(SynapticError::Tool(format!(
            "Tool '{}' is not in the allowed list: {:?}",
            request.call.name, allowed_tools,
        )));
    }
    next.call(request).await
}

// --- Audit: before agent ---
// Log the number of messages when the agent starts.

#[before_agent]
async fn audit_start(
    #[field] label: String,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    println!("[{}] Agent starting with {} messages", label, messages.len());
    Ok(())
}

// --- Audit: after agent ---
// Log the number of messages when the agent finishes.

#[after_agent]
async fn audit_end(
    #[field] label: String,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    println!("[{}] Agent completed with {} messages", label, messages.len());
    Ok(())
}

// --- Assemble the middleware stack ---

fn build_secured_stack() -> Vec<Arc<dyn AgentMiddleware>> {
    let allowed = vec![
        "web_search".to_string(),
        "get_weather".to_string(),
    ];

    vec![
        audit_start("prod-agent".into()),
        permission_gate(allowed),
        audit_end("prod-agent".into()),
    ]
}
```

**Key takeaways:**

- `#[wrap_tool_call]` gives full control over tool execution. Check
  permissions, transform arguments, or deny the call entirely by returning
  an error instead of calling `next.call()`.
- `#[before_agent]` and `#[after_agent]` bracket the entire agent lifecycle,
  making them ideal for audit logging, metrics collection, or resource
  setup/teardown.
- `#[field]` makes each middleware configurable and reusable. The
  `permission_gate` can be instantiated with different allowlists for
  different agents, and the audit middleware accepts a label for log
  disambiguation.

### Scenario G: State-Aware Tool with Raw Arguments

This scenario demonstrates `#[inject(state)]` for reading graph state and
`#[args]` for accepting raw JSON payloads, plus a combination of both
patterns with `#[field]`.

```rust,ignore
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use synaptic::core::SynapticError;
use synaptic::macros::tool;

// --- State-aware tool ---
// Reads the graph state to adjust its behavior. After 10 conversation
// turns the tool switches to shorter replies.

#[derive(Deserialize)]
struct ConversationState {
    turn_count: usize,
}

/// Generate a context-aware reply.
#[tool]
async fn smart_reply(
    /// The user's latest message
    message: String,
    #[inject(state)]
    state: ConversationState,
) -> Result<String, SynapticError> {
    if state.turn_count > 10 {
        // After 10 turns, keep it short
        Ok(format!("TL;DR: {}", &message[..message.len().min(50)]))
    } else {
        Ok(format!(
            "Turn {}: Let me elaborate on '{}'...",
            state.turn_count, message
        ))
    }
}

// --- Raw-args JSON proxy ---
// Accepts any JSON payload and forwards it to a webhook endpoint.
// No schema is generated -- the LLM sends whatever JSON it wants.

/// Forward a JSON payload to an external webhook.
#[tool(name = "webhook_forward")]
async fn webhook_forward(#[args] payload: Value) -> Result<String, SynapticError> {
    // In production: reqwest::Client::new().post(url).json(&payload).send().await
    Ok(format!("Forwarded payload with {} keys", payload.as_object().map_or(0, |m| m.len())))
}

// --- Configurable API proxy ---
// Combines #[field] for a base endpoint with #[args] for the request body.
// Each instance points at a different API.

/// Proxy arbitrary JSON to a configured API endpoint.
#[tool(name = "api_proxy")]
async fn api_proxy(
    #[field] endpoint: String,
    #[args] body: Value,
) -> Result<String, SynapticError> {
    // In production: reqwest::Client::new().post(&endpoint).json(&body).send().await
    Ok(format!(
        "POST {} with {} bytes",
        endpoint,
        body.to_string().len()
    ))
}

fn main() {
    // State-aware tool -- the LLM only sees "message" in the schema
    let reply_tool = smart_reply();

    // Raw-args tool -- parameters() returns None
    let webhook_tool = webhook_forward();

    // Configurable proxy -- each instance targets a different endpoint
    let users_api = api_proxy("https://api.example.com/users".into());
    let orders_api = api_proxy("https://api.example.com/orders".into());
}
```

**Key takeaways:**

- `#[inject(state)]` gives tools read access to the current graph state
  without exposing it to the LLM. The state is deserialized from
  `ToolRuntime::state` into your custom struct automatically.
- `#[args]` bypasses schema generation entirely -- the tool accepts whatever
  JSON the LLM sends. Use this for proxy/forwarding patterns or tools that
  handle arbitrary payloads. `parameters()` returns `None` when `#[args]` is
  the only non-field, non-inject parameter.
- `#[field]` + `#[args]` combine naturally. The field is provided at
  construction time (hidden from the LLM), while the raw JSON arrives at
  call time. This makes it easy to create reusable tool templates that
  differ only in configuration.

---

## Comparison with Python LangChain

If you are coming from Python LangChain / LangGraph, here is how the Synaptic
macros map to their Python equivalents:

| Python | Rust (Synaptic) | Notes |
|--------|----------------|-------|
| `@tool` | `#[tool]` | Both generate a tool from a function; Rust version produces a struct + trait impl |
| `RunnableLambda(fn)` | `#[chain]` | Rust version returns `BoxRunnable<I, O>` with auto-detected output type |
| `@entrypoint` | `#[entrypoint]` | Both define a workflow entry point; Rust adds `checkpointer` hint |
| `@task` | `#[task]` | Both mark a function as a named sub-task |
| Middleware classes | `#[before_agent]`, `#[before_model]`, `#[after_model]`, `#[after_agent]`, `#[wrap_model_call]`, `#[wrap_tool_call]`, `#[dynamic_prompt]` | Rust splits each hook into its own macro for clarity |
| `@traceable` | `#[traceable]` | Rust uses `tracing` crate spans; Python uses LangSmith |
| `InjectedState`, `InjectedStore`, `InjectedToolCallId` | `#[inject(state)]`, `#[inject(store)]`, `#[inject(tool_call_id)]` | Rust uses parameter-level attributes instead of type annotations |

---

## How Tool Definitions Reach the LLM

Understanding the full journey from a Rust function to an LLM tool call helps
debug schema issues and customize behavior. Here is the complete chain:

```text
#[tool] macro
    │
    ▼
struct + impl Tool    (generated at compile time)
    │
    ▼
tool.as_tool_definition() → ToolDefinition { name, description, parameters }
    │
    ▼
ChatRequest::with_tools(vec![...])    (tool definitions attached to request)
    │
    ▼
Model Adapter (OpenAI / Anthropic / Gemini)
    │   Converts ToolDefinition → provider-specific JSON
    │   e.g. OpenAI: {"type": "function", "function": {"name": ..., "parameters": ...}}
    ▼
HTTP POST → LLM API
    │
    ▼
LLM returns ToolCall { id, name, arguments }
    │
    ▼
ToolNode dispatches → tool.call(arguments)
    │
    ▼
Tool Message back into conversation
```

**Key files in the codebase:**

| Step | File |
|------|------|
| `#[tool]` macro expansion | `crates/synaptic-macros/src/tool.rs` |
| `Tool` / `RuntimeAwareTool` traits | `crates/synaptic-core/src/lib.rs` |
| `ToolDefinition`, `ToolCall` types | `crates/synaptic-core/src/lib.rs` |
| `ToolNode` (dispatches calls) | `crates/synaptic-graph/src/tool_node.rs` |
| OpenAI adapter | `crates/synaptic-models/src/openai.rs` |
| Anthropic adapter | `crates/synaptic-models/src/anthropic.rs` |
| Gemini adapter | `crates/synaptic-models/src/gemini.rs` |

## Testing Macro-Generated Code

Tools generated by `#[tool]` can be tested like any other `Tool` implementation. Call `as_tool_definition()` to inspect the schema and `call()` to verify behavior:

```rust,ignore
use serde_json::json;
use synaptic::core::Tool;

/// Add two numbers.
#[tool]
async fn add(
    /// The first number
    a: f64,
    /// The second number
    b: f64,
) -> Result<serde_json::Value, SynapticError> {
    Ok(json!({"result": a + b}))
}

#[tokio::test]
async fn test_add_tool() {
    let tool = add();

    // Verify metadata
    assert_eq!(tool.name(), "add");
    assert_eq!(tool.description(), "Add two numbers.");

    // Verify schema
    let def = tool.as_tool_definition();
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("a")));
    assert!(required.contains(&json!("b")));

    // Verify execution
    let result = tool.call(json!({"a": 3.0, "b": 4.0})).await.unwrap();
    assert_eq!(result["result"], 7.0);
}
```

For `#[chain]` macros, test the returned `BoxRunnable` with `invoke()`:

```rust,ignore
use synaptic::core::RunnableConfig;
use synaptic::runnables::Runnable;

#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[tokio::test]
async fn test_chain() {
    let runnable = to_upper();
    let config = RunnableConfig::default();
    let result = runnable.invoke("hello".into(), &config).await.unwrap();
    assert_eq!(result, "HELLO");
}
```

### What can go wrong

1. **Custom types without `schemars`**: The parameter schema is `{"type": "object"}`
   with no field details. The LLM guesses (often incorrectly) what to send.
   **Fix**: Enable the `schemars` feature and derive `JsonSchema`.

2. **Missing `as_tool_definition()` call**: If you construct `ToolDefinition`
   manually with `json!({})` for parameters instead of calling
   `tool.as_tool_definition()`, the schema will be empty.
   **Fix**: Always use `as_tool_definition()` on your `Tool` / `RuntimeAwareTool`.

3. **OpenAI strict mode**: OpenAI's function calling strict mode rejects schemas
   with missing `type` fields. All built-in types and `Value` now generate valid
   schemas with `"type"` specified.
