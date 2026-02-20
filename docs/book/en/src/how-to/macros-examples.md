# Macro Examples

The following end-to-end scenarios show how the macros work together in
realistic applications.

## Scenario A: Weather Agent with Custom Tool

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

## Scenario B: Data Pipeline with Chain Macros

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

## Scenario C: Agent with Middleware Stack

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

## Scenario D: Store-Backed Note Manager with Typed Input

This example combines `#[inject]` for runtime access and `schemars` for rich
JSON Schema generation. A `save_note` tool accepts a custom `NoteInput` struct
whose full schema (title, content, tags) is visible to the LLM, while the
shared store and tool call ID are injected transparently by the agent runtime.

**Cargo.toml** -- enable the `agent`, `store`, and `schemars` features:

```toml
[dependencies]
synaptic = { version = "0.1", features = ["agent", "store", "schemars"] }
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

## Scenario E: Workflow with Entrypoint, Tasks, and Tracing

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

## Scenario F: Tool Permission Gating with Audit Logging

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

## Scenario G: State-Aware Tool with Raw Arguments

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
    |
    v
struct + impl Tool    (generated at compile time)
    |
    v
tool.as_tool_definition() -> ToolDefinition { name, description, parameters }
    |
    v
ChatRequest::with_tools(vec![...])    (tool definitions attached to request)
    |
    v
Model Adapter (OpenAI / Anthropic / Gemini)
    |   Converts ToolDefinition -> provider-specific JSON
    |   e.g. OpenAI: {"type": "function", "function": {"name": ..., "parameters": ...}}
    v
HTTP POST -> LLM API
    |
    v
LLM returns ToolCall { id, name, arguments }
    |
    v
ToolNode dispatches -> tool.call(arguments)
    |
    v
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
