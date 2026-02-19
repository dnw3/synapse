//! Comprehensive showcase of all proc macros from the `synaptic-macros` crate.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p macros_showcase
//! ```
//!
//! This example demonstrates 12 attribute macros and several usage variants:
//!
//!  1. `#[tool]`            -- basic tool from a function
//!  2. `#[tool]` + `#[default]` -- tool with default parameter values
//!  3. `#[tool]` + `Option<T>`  -- tool with optional parameters
//!  4. `#[tool(name = "...")]`  -- tool with a custom name
//!  5. `#[tool]` + `#[inject(state)]` -- RuntimeAwareTool that injects state
//!  6. `#[chain]`           -- convert a function to a BoxRunnable
//!  7. `#[entrypoint]`      -- create a workflow entrypoint
//!  8. `#[task]`            -- define a trackable task
//!  9. `#[before_agent]`    -- before-agent middleware
//! 10. `#[before_model]`    -- before-model middleware
//! 11. `#[after_model]`     -- after-model middleware
//! 12. `#[after_agent]`     -- after-agent middleware
//! 13. `#[dynamic_prompt]`  -- dynamic system prompt middleware
//! 14. `#[traceable]`       -- tracing instrumentation
//! 15. `#[traceable(skip)]` -- tracing with skipped params

use std::sync::Arc;

use serde_json::{json, Value};
use synaptic_core::{Message, RunnableConfig, RuntimeAwareTool, SynapticError, ToolRuntime};
use synaptic_macros::{
    after_agent, after_model, before_agent, before_model, chain, dynamic_prompt, entrypoint, task,
    tool, traceable,
};
use synaptic_middleware::{AgentMiddleware, ModelRequest, ModelResponse};
use synaptic_runnables::Runnable;

// ============================================================================
// 1. #[tool] -- Basic tool
// ============================================================================

/// Search the web for information.
#[tool]
async fn web_search(
    /// The search query string
    query: String,
) -> Result<String, SynapticError> {
    Ok(format!("Results for: {}", query))
}

// ============================================================================
// 2. #[tool] with #[default] -- Tool with default parameter values
// ============================================================================

/// Multiply two numbers with an optional scale factor.
#[tool]
async fn multiply(
    /// The first operand
    a: f64,
    /// The second operand
    b: f64,
    /// Scale factor applied to the product
    #[default = 1.0]
    scale: f64,
) -> Result<f64, SynapticError> {
    Ok(a * b * scale)
}

// ============================================================================
// 3. #[tool] with Option<T> -- Tool with optional parameters
// ============================================================================

/// Greet someone with an optional title.
#[tool]
async fn greet(
    /// The person's name
    name: String,
    /// Optional honorific (e.g. "Dr.", "Prof.")
    title: Option<String>,
) -> Result<String, SynapticError> {
    match title {
        Some(t) => Ok(format!("Hello, {} {}!", t, name)),
        None => Ok(format!("Hello, {}!", name)),
    }
}

// ============================================================================
// 4. #[tool(name = "...")] -- Custom tool name
// ============================================================================

/// Evaluate a mathematical expression.
#[tool(name = "calculator")]
async fn calc(
    /// The expression to evaluate (e.g. "2 + 2")
    expression: String,
) -> Result<String, SynapticError> {
    Ok(format!("Calculated: {}", expression))
}

// ============================================================================
// 5. #[tool] with #[inject(state)] -- RuntimeAwareTool
// ============================================================================

/// Look up a value from the agent state by key.
#[tool]
async fn state_lookup(
    /// The key to look up in the state
    key: String,
    #[inject(state)] state: Value,
) -> Result<String, SynapticError> {
    let val = state
        .get(&key)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "not found".into());
    Ok(val)
}

// ============================================================================
// 6. #[chain] -- Convert a function to a BoxRunnable
// ============================================================================

#[chain]
async fn to_uppercase(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default().to_uppercase();
    Ok(Value::String(s))
}

#[chain]
async fn add_exclamation(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default();
    Ok(Value::String(format!("{}!", s)))
}

// ============================================================================
// 7. #[entrypoint] -- Create a workflow entrypoint
// ============================================================================

#[entrypoint(checkpointer = "memory")]
async fn my_workflow(input: Value) -> Result<Value, SynapticError> {
    let name = input
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("World");
    Ok(json!({ "greeting": format!("Hello, {}!", name) }))
}

// ============================================================================
// 8. #[task] -- Define a trackable task
// ============================================================================

#[task(name = "weather_fetcher")]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    // In a real application this would call a weather API.
    Ok(format!("Sunny, 25C in {}", city))
}

// ============================================================================
// 9. #[before_agent] -- Before-agent middleware
// ============================================================================

#[before_agent]
async fn inject_system_message(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    messages.insert(0, Message::system("You are a helpful assistant."));
    Ok(())
}

// ============================================================================
// 10. #[before_model] -- Before-model middleware
// ============================================================================

#[before_model]
async fn set_system_prompt(request: &mut ModelRequest) -> Result<(), SynapticError> {
    request.system_prompt = Some("Always be concise.".into());
    Ok(())
}

// ============================================================================
// 11. #[after_model] -- After-model middleware
// ============================================================================

#[after_model]
async fn log_model_response(
    _request: &ModelRequest,
    response: &mut ModelResponse,
) -> Result<(), SynapticError> {
    println!(
        "  [after_model] Model responded with: \"{}\"",
        response.message.content()
    );
    Ok(())
}

// ============================================================================
// 12. #[after_agent] -- After-agent middleware
// ============================================================================

#[after_agent]
async fn append_done_marker(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    messages.push(Message::system("[agent loop finished]"));
    Ok(())
}

// ============================================================================
// 13. #[dynamic_prompt] -- Dynamic system prompt middleware
// ============================================================================

#[dynamic_prompt]
fn context_aware_prompt(messages: &[Message]) -> String {
    format!(
        "You have {} messages in context. Respond accordingly.",
        messages.len()
    )
}

// ============================================================================
// 14. #[traceable] -- Tracing instrumentation
// ============================================================================

#[traceable]
async fn process_data(input: String, count: usize) -> String {
    format!("{} (x{})", input, count)
}

// ============================================================================
// 15. #[traceable(skip = "...")] -- Tracing with skipped params
// ============================================================================

#[traceable(name = "auth_check", skip = "api_key")]
async fn authenticate(user: String, api_key: String) -> bool {
    // The api_key parameter is excluded from the tracing span fields
    // so it will not appear in logs.
    !user.is_empty() && !api_key.is_empty()
}

// ============================================================================
// main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize a tracing subscriber so #[traceable] spans are visible.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== synaptic-macros showcase ===\n");

    // ------------------------------------------------------------------
    // 1. #[tool] -- Basic tool
    // ------------------------------------------------------------------
    println!("--- 1. #[tool] (basic) ---");
    let t = web_search();
    println!("  name:        {}", t.name());
    println!("  description: {}", t.description());
    println!("  schema:      {}", t.parameters().unwrap());
    let result = t.call(json!({"query": "Rust language"})).await.unwrap();
    println!("  call result: {}\n", result);

    // ------------------------------------------------------------------
    // 2. #[tool] with #[default]
    // ------------------------------------------------------------------
    println!("--- 2. #[tool] with #[default] ---");
    let t = multiply();
    println!("  name:   {}", t.name());
    println!("  schema: {}", t.parameters().unwrap());
    let r1 = t.call(json!({"a": 3.0, "b": 4.0})).await.unwrap();
    println!("  3 * 4 (default scale=1): {}", r1);
    let r2 = t
        .call(json!({"a": 3.0, "b": 4.0, "scale": 10.0}))
        .await
        .unwrap();
    println!("  3 * 4 * 10:              {}\n", r2);

    // ------------------------------------------------------------------
    // 3. #[tool] with Option<T>
    // ------------------------------------------------------------------
    println!("--- 3. #[tool] with Option<T> ---");
    let t = greet();
    println!("  schema: {}", t.parameters().unwrap());
    let r1 = t.call(json!({"name": "Alice"})).await.unwrap();
    println!("  without title: {}", r1);
    let r2 = t
        .call(json!({"name": "Alice", "title": "Dr."}))
        .await
        .unwrap();
    println!("  with title:    {}\n", r2);

    // ------------------------------------------------------------------
    // 4. #[tool(name = "...")]
    // ------------------------------------------------------------------
    println!("--- 4. #[tool(name = \"calculator\")] ---");
    let t = calc();
    println!("  name (custom): {}", t.name());
    let result = t.call(json!({"expression": "2 + 2"})).await.unwrap();
    println!("  call result:   {}\n", result);

    // ------------------------------------------------------------------
    // 5. #[tool] with #[inject(state)]
    // ------------------------------------------------------------------
    println!("--- 5. #[tool] with #[inject(state)] (RuntimeAwareTool) ---");
    let t: Arc<dyn RuntimeAwareTool> = state_lookup();
    println!("  name:   {}", t.name());
    println!("  schema: {}", t.parameters().unwrap());
    let runtime = ToolRuntime {
        store: None,
        stream_writer: None,
        state: Some(json!({"user": "Alice", "role": "admin"})),
        tool_call_id: "call_001".to_string(),
        config: None,
    };
    let result = t
        .call_with_runtime(json!({"key": "user"}), runtime)
        .await
        .unwrap();
    println!("  lookup 'user': {}\n", result);

    // ------------------------------------------------------------------
    // 6. #[chain]
    // ------------------------------------------------------------------
    println!("--- 6. #[chain] ---");
    let upper = to_uppercase();
    let config = RunnableConfig::default();
    let result = upper.invoke(json!("hello world"), &config).await.unwrap();
    println!("  to_uppercase: {}", result);

    // Chains can be composed with the pipe operator:
    let pipeline = to_uppercase() | add_exclamation();
    let result = pipeline.invoke(json!("hello"), &config).await.unwrap();
    println!("  to_uppercase | add_exclamation: {}\n", result);

    // ------------------------------------------------------------------
    // 7. #[entrypoint]
    // ------------------------------------------------------------------
    println!("--- 7. #[entrypoint] ---");
    let ep = my_workflow();
    println!("  config.name:          {}", ep.config.name);
    println!("  config.checkpointer:  {:?}", ep.config.checkpointer);
    let result = ep.invoke(json!({"name": "Synaptic"})).await.unwrap();
    println!("  invoke result:        {}\n", result);

    // ------------------------------------------------------------------
    // 8. #[task]
    // ------------------------------------------------------------------
    println!("--- 8. #[task] ---");
    let weather = fetch_weather("Tokyo".to_string()).await.unwrap();
    println!("  fetch_weather(\"Tokyo\"): {}\n", weather);

    // ------------------------------------------------------------------
    // 9. #[before_agent]
    // ------------------------------------------------------------------
    println!("--- 9. #[before_agent] ---");
    let mw: Arc<dyn AgentMiddleware> = inject_system_message();
    let mut messages = vec![Message::human("What is Rust?")];
    mw.before_agent(&mut messages).await.unwrap();
    println!("  messages after before_agent:");
    for msg in &messages {
        println!("    [{}] {}", msg.role(), msg.content());
    }
    println!();

    // ------------------------------------------------------------------
    // 10. #[before_model]
    // ------------------------------------------------------------------
    println!("--- 10. #[before_model] ---");
    let mw: Arc<dyn AgentMiddleware> = set_system_prompt();
    let mut req = ModelRequest {
        messages: vec![Message::human("Hi")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    println!(
        "  system_prompt after before_model: {:?}\n",
        req.system_prompt
    );

    // ------------------------------------------------------------------
    // 11. #[after_model]
    // ------------------------------------------------------------------
    println!("--- 11. #[after_model] ---");
    let mw: Arc<dyn AgentMiddleware> = log_model_response();
    let req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    let mut resp = ModelResponse {
        message: Message::ai("I am a helpful AI."),
        usage: None,
    };
    mw.after_model(&req, &mut resp).await.unwrap();
    println!();

    // ------------------------------------------------------------------
    // 12. #[after_agent]
    // ------------------------------------------------------------------
    println!("--- 12. #[after_agent] ---");
    let mw: Arc<dyn AgentMiddleware> = append_done_marker();
    let mut messages = vec![Message::ai("Final answer.")];
    mw.after_agent(&mut messages).await.unwrap();
    println!("  messages after after_agent:");
    for msg in &messages {
        println!("    [{}] {}", msg.role(), msg.content());
    }
    println!();

    // ------------------------------------------------------------------
    // 13. #[dynamic_prompt]
    // ------------------------------------------------------------------
    println!("--- 13. #[dynamic_prompt] ---");
    let mw: Arc<dyn AgentMiddleware> = context_aware_prompt();
    let mut req = ModelRequest {
        messages: vec![
            Message::human("first"),
            Message::ai("reply"),
            Message::human("second"),
        ],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    println!("  dynamic system_prompt: {:?}\n", req.system_prompt);

    // ------------------------------------------------------------------
    // 14. #[traceable] -- basic
    // ------------------------------------------------------------------
    println!("--- 14. #[traceable] ---");
    let result = process_data("traced-data".into(), 3).await;
    println!("  process_data result: {}", result);
    println!("  (check tracing output above for the info_span)\n");

    // ------------------------------------------------------------------
    // 15. #[traceable(skip = "...")] -- with skipped params
    // ------------------------------------------------------------------
    println!("--- 15. #[traceable(skip = \"api_key\")] ---");
    let ok = authenticate("alice".into(), "super_secret_key".into()).await;
    println!("  authenticate result: {}", ok);
    println!("  (api_key is NOT recorded in the tracing span)\n");

    println!("=== all 15 demonstrations complete ===");
}
