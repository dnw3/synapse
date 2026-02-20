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

| Macro | Purpose | Page |
|-------|---------|------|
| `#[tool]` | Define tools from functions | This page |
| `#[chain]` | Create runnable chains | This page |
| `#[entrypoint]` | Workflow entry points | This page |
| `#[task]` | Trackable tasks | This page |
| `#[traceable]` | Tracing instrumentation | This page |
| `#[before_agent]` | Middleware: before agent loop | [Middleware Macros](macros-middleware.md) |
| `#[before_model]` | Middleware: before model call | [Middleware Macros](macros-middleware.md) |
| `#[after_model]` | Middleware: after model call | [Middleware Macros](macros-middleware.md) |
| `#[after_agent]` | Middleware: after agent loop | [Middleware Macros](macros-middleware.md) |
| `#[wrap_model_call]` | Middleware: wrap model call | [Middleware Macros](macros-middleware.md) |
| `#[wrap_tool_call]` | Middleware: wrap tool call | [Middleware Macros](macros-middleware.md) |
| `#[dynamic_prompt]` | Middleware: dynamic system prompt | [Middleware Macros](macros-middleware.md) |

For complete end-to-end scenarios, see [Macro Examples](macros-examples.md).

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
