//! Procedural macros for the Synaptic framework.
//!
//! This crate provides attribute macros that reduce boilerplate when defining
//! tools, runnable chains, graph entrypoints, tasks, middleware hooks, and
//! traced functions.
//!
//! # Macros
//!
//! | Macro | Description |
//! |-------|-------------|
//! | [`#[tool]`](macro@tool) | Convert an async fn into a `Tool` implementor |
//! | [`#[chain]`](macro@chain) | Convert an async fn into a `BoxRunnable` |
//! | [`#[entrypoint]`](macro@entrypoint) | Define a LangGraph-style workflow entry point |
//! | [`#[task]`](macro@task) | Define a trackable task inside an entrypoint |
//! | [`#[before_agent]`](macro@before_agent) | Middleware: before agent loop |
//! | [`#[before_model]`](macro@before_model) | Middleware: before model call |
//! | [`#[after_model]`](macro@after_model) | Middleware: after model call |
//! | [`#[after_agent]`](macro@after_agent) | Middleware: after agent loop |
//! | [`#[wrap_model_call]`](macro@wrap_model_call) | Middleware: wrap model call |
//! | [`#[wrap_tool_call]`](macro@wrap_tool_call) | Middleware: wrap tool call |
//! | [`#[dynamic_prompt]`](macro@dynamic_prompt) | Middleware: dynamic system prompt |
//! | [`#[traceable]`](macro@traceable) | Add tracing instrumentation |

extern crate proc_macro;

mod chain;
mod entrypoint;
mod middleware;
mod paths;
mod task;
mod tool;
mod traceable;

use proc_macro::TokenStream;

/// Convert an async function into a struct that implements `synaptic_core::Tool`.
///
/// # Features
///
/// - Function name becomes the tool name
/// - Doc comments become the tool description
/// - Parameters are mapped to a JSON Schema automatically
/// - `Option<T>` parameters are optional (not in `required`)
/// - `#[default = value]` sets a default for a parameter
/// - `#[inject(state)]`, `#[inject(store)]`, `#[inject(tool_call_id)]` inject
///   runtime values (the parameter is hidden from the LLM schema); using any
///   inject attribute switches the generated impl to `RuntimeAwareTool`
/// - Parameter doc comments become `"description"` in the schema
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::tool;
/// use synaptic_core::SynapticError;
///
/// /// Search the web for information.
/// #[tool]
/// async fn search(
///     /// The search query
///     query: String,
///     /// Maximum number of results
///     #[default = 5]
///     max_results: i64,
/// ) -> Result<String, SynapticError> {
///     Ok(format!("Searching for '{}' (max {})", query, max_results))
/// }
///
/// // `search` is now a function returning `Arc<dyn Tool>`
/// let tool = search();
/// ```
///
/// # Custom name
///
/// ```ignore
/// #[tool(name = "web_search")]
/// async fn search(query: String) -> Result<String, SynapticError> {
///     Ok(format!("Searching for '{}'", query))
/// }
/// ```
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::expand_tool(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Convert an async function into a `BoxRunnable<InputType, OutputType>` factory.
///
/// The macro generates a public function with the same name that returns a
/// `BoxRunnable` backed by a `RunnableLambda`.
///
/// The output type is inferred from the function signature:
/// - `Result<Value, _>` → `BoxRunnable<I, Value>` (serializes to Value)
/// - `Result<String, _>` → `BoxRunnable<I, String>` (direct return)
/// - `Result<T, _>` → `BoxRunnable<I, T>` (direct return)
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::chain;
/// use synaptic_core::SynapticError;
/// use serde_json::Value;
///
/// #[chain]
/// async fn uppercase(input: Value) -> Result<Value, SynapticError> {
///     let s = input.as_str().unwrap_or_default().to_uppercase();
///     Ok(Value::String(s))
/// }
///
/// // Returns BoxRunnable<Value, Value>
/// let runnable = uppercase();
///
/// // Typed output — returns BoxRunnable<String, String>
/// #[chain]
/// async fn to_upper(s: String) -> Result<String, SynapticError> {
///     Ok(s.to_uppercase())
/// }
/// ```
#[proc_macro_attribute]
pub fn chain(attr: TokenStream, item: TokenStream) -> TokenStream {
    chain::expand_chain(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Define a LangGraph-style workflow entry point.
///
/// Converts an async function that takes `serde_json::Value` and returns
/// `Result<serde_json::Value, SynapticError>` into a factory function that
/// returns an [`Entrypoint`](::synaptic_core::Entrypoint).
///
/// # Attributes
///
/// - `name = "..."` — override the entrypoint name (defaults to the function name)
/// - `checkpointer = "..."` — hint which checkpointer backend to use (e.g. `"memory"`)
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::entrypoint;
/// use synaptic_core::SynapticError;
/// use serde_json::Value;
///
/// #[entrypoint(checkpointer = "memory")]
/// async fn my_workflow(input: Value) -> Result<Value, SynapticError> {
///     Ok(input)
/// }
///
/// // `my_workflow` is now a function returning `Entrypoint`
/// let ep = my_workflow();
/// ```
#[proc_macro_attribute]
pub fn entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    entrypoint::expand_entrypoint(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Define a trackable task inside an entrypoint.
///
/// Wraps an async function so that it carries a task name for tracing and
/// streaming identification. The original function body is moved into a
/// private `{name}_impl` helper and a public wrapper delegates to it.
///
/// # Attributes
///
/// - `name = "..."` — override the task name (defaults to the function name)
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::task;
/// use synaptic_core::SynapticError;
///
/// #[task]
/// async fn fetch_weather(city: String) -> Result<String, SynapticError> {
///     Ok(format!("Sunny in {}", city))
/// }
///
/// // `fetch_weather` can be called directly — it forwards to `fetch_weather_impl`
/// let result = fetch_weather("Paris".into()).await;
/// ```
#[proc_macro_attribute]
pub fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    task::expand_task(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: run a hook before the agent loop starts.
///
/// The decorated async function must accept `&mut Vec<Message>` and return
/// `Result<(), SynapticError>`. The macro generates a struct that implements
/// `AgentMiddleware` with only `before_agent` overridden, plus a factory
/// function returning `Arc<dyn AgentMiddleware>`.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::before_agent;
/// use synaptic_core::{Message, SynapticError};
///
/// #[before_agent]
/// async fn setup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
///     println!("Agent starting with {} messages", messages.len());
///     Ok(())
/// }
///
/// let mw = setup(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn before_agent(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_before_agent(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: run a hook before each model call.
///
/// The decorated async function must accept `&mut ModelRequest` and return
/// `Result<(), SynapticError>`.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::before_model;
/// use synaptic_middleware::ModelRequest;
/// use synaptic_core::SynapticError;
///
/// #[before_model]
/// async fn add_context(request: &mut ModelRequest) -> Result<(), SynapticError> {
///     request.system_prompt = Some("Be helpful".into());
///     Ok(())
/// }
///
/// let mw = add_context(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn before_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_before_model(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: run a hook after each model call.
///
/// The decorated async function must accept `&ModelRequest` and
/// `&mut ModelResponse`, returning `Result<(), SynapticError>`.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::after_model;
/// use synaptic_middleware::{ModelRequest, ModelResponse};
/// use synaptic_core::SynapticError;
///
/// #[after_model]
/// async fn log_response(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError> {
///     println!("Model responded: {}", response.message.content());
///     Ok(())
/// }
///
/// let mw = log_response(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn after_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_after_model(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: run a hook after the agent loop finishes.
///
/// The decorated async function must accept `&mut Vec<Message>` and return
/// `Result<(), SynapticError>`.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::after_agent;
/// use synaptic_core::{Message, SynapticError};
///
/// #[after_agent]
/// async fn cleanup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
///     println!("Agent done");
///     Ok(())
/// }
///
/// let mw = cleanup(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn after_agent(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_after_agent(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: wrap the model call with custom logic.
///
/// The decorated async function must accept `ModelRequest` and
/// `&dyn ModelCaller`, returning `Result<ModelResponse, SynapticError>`.
/// This enables retry, fallback, and other wrapping patterns.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::wrap_model_call;
/// use synaptic_middleware::{ModelRequest, ModelResponse, ModelCaller};
/// use synaptic_core::SynapticError;
///
/// #[wrap_model_call]
/// async fn retry_model(request: ModelRequest, next: &dyn ModelCaller) -> Result<ModelResponse, SynapticError> {
///     match next.call(request.clone()).await {
///         Ok(r) => Ok(r),
///         Err(_) => next.call(request).await,
///     }
/// }
///
/// let mw = retry_model(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn wrap_model_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_wrap_model_call(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: wrap a tool call with custom logic.
///
/// The decorated async function must accept `ToolCallRequest` and
/// `&dyn ToolCaller`, returning `Result<Value, SynapticError>`.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::wrap_tool_call;
/// use synaptic_middleware::{ToolCallRequest, ToolCaller};
/// use synaptic_core::SynapticError;
/// use serde_json::Value;
///
/// #[wrap_tool_call]
/// async fn log_tool(request: ToolCallRequest, next: &dyn ToolCaller) -> Result<Value, SynapticError> {
///     println!("Calling tool: {}", request.call.name);
///     next.call(request).await
/// }
///
/// let mw = log_tool(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn wrap_tool_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_wrap_tool_call(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Middleware: dynamically generate a system prompt based on current messages.
///
/// The decorated function (non-async) must accept `&[Message]` and return
/// `String`. The macro generates a middleware whose `before_model` hook
/// sets `request.system_prompt` to the return value.
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::dynamic_prompt;
/// use synaptic_core::Message;
///
/// #[dynamic_prompt]
/// fn custom_prompt(messages: &[Message]) -> String {
///     format!("You have {} messages in context", messages.len())
/// }
///
/// let mw = custom_prompt(); // Arc<dyn AgentMiddleware>
/// ```
#[proc_macro_attribute]
pub fn dynamic_prompt(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::expand_dynamic_prompt(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Add tracing instrumentation to an async or sync function.
///
/// Wraps the function body in a `tracing::info_span!` with the function name
/// and parameter values recorded as span fields. Async functions use
/// `tracing::Instrument` for correct span propagation.
///
/// # Attributes
///
/// - `name = "..."` — override the span name (defaults to the function name)
/// - `skip = "a,b"` — comma-separated list of parameter names to exclude from the span
///
/// # Example
///
/// ```ignore
/// use synaptic_macros::traceable;
///
/// #[traceable]
/// async fn process_data(input: String, count: usize) -> String {
///     format!("{}: {}", input, count)
/// }
///
/// #[traceable(name = "custom_span", skip = "secret")]
/// async fn with_secret(query: String, secret: String) -> String {
///     format!("Processing: {}", query)
/// }
/// ```
#[proc_macro_attribute]
pub fn traceable(attr: TokenStream, item: TokenStream) -> TokenStream {
    traceable::expand_traceable(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
