//! Integration tests for the middleware attribute macros.

use std::sync::Arc;

use synaptic_core::{Message, SynapticError};
use synaptic_macros::{
    after_agent, after_model, before_agent, before_model, dynamic_prompt, wrap_model_call,
};
use synaptic_middleware::{AgentMiddleware, ModelCaller, ModelRequest, ModelResponse};

// ---------------------------------------------------------------------------
// #[before_agent]
// ---------------------------------------------------------------------------

#[before_agent]
async fn setup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    messages.push(Message::system("setup ran"));
    Ok(())
}

#[tokio::test]
async fn test_before_agent_middleware() {
    let mw: Arc<dyn AgentMiddleware> = setup();
    let mut messages = vec![Message::human("hello")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "setup ran");
}

// ---------------------------------------------------------------------------
// #[before_model]
// ---------------------------------------------------------------------------

#[before_model]
async fn add_context(request: &mut ModelRequest) -> Result<(), SynapticError> {
    request.system_prompt = Some("Be helpful".into());
    Ok(())
}

#[tokio::test]
async fn test_before_model_middleware() {
    let mw: Arc<dyn AgentMiddleware> = add_context();
    let mut req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    assert_eq!(req.system_prompt.as_deref(), Some("Be helpful"));
}

// ---------------------------------------------------------------------------
// #[after_model]
// ---------------------------------------------------------------------------

#[after_model]
async fn log_response(
    _request: &ModelRequest,
    response: &mut ModelResponse,
) -> Result<(), SynapticError> {
    // Simulate modifying the response by replacing the message
    response.message = Message::ai(format!("logged: {}", response.message.content()));
    Ok(())
}

#[tokio::test]
async fn test_after_model_middleware() {
    let mw: Arc<dyn AgentMiddleware> = log_response();
    let req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    let mut resp = ModelResponse {
        message: Message::ai("original"),
        usage: None,
    };
    mw.after_model(&req, &mut resp).await.unwrap();
    assert_eq!(resp.message.content(), "logged: original");
}

// ---------------------------------------------------------------------------
// #[after_agent]
// ---------------------------------------------------------------------------

#[after_agent]
async fn cleanup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    messages.push(Message::system("cleanup ran"));
    Ok(())
}

#[tokio::test]
async fn test_after_agent_middleware() {
    let mw: Arc<dyn AgentMiddleware> = cleanup();
    let mut messages = vec![Message::ai("done")];
    mw.after_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "cleanup ran");
}

// ---------------------------------------------------------------------------
// #[dynamic_prompt]
// ---------------------------------------------------------------------------

#[dynamic_prompt]
fn custom_prompt(messages: &[Message]) -> String {
    format!("You have {} messages in context", messages.len())
}

#[tokio::test]
async fn test_dynamic_prompt_middleware() {
    let mw: Arc<dyn AgentMiddleware> = custom_prompt();
    let mut req = ModelRequest {
        messages: vec![Message::human("hi"), Message::ai("hello")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    assert_eq!(
        req.system_prompt.as_deref(),
        Some("You have 2 messages in context")
    );
}

// ---------------------------------------------------------------------------
// Verify middleware struct names (factory returns Arc)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_middleware_name() {
    // The factory functions return Arc<dyn AgentMiddleware>, which proves
    // that the generated structs (SetupMiddleware, AddContextMiddleware, etc.)
    // correctly implement the AgentMiddleware trait.
    let _setup: Arc<dyn AgentMiddleware> = setup();
    let _add_context: Arc<dyn AgentMiddleware> = add_context();
    let _log_response: Arc<dyn AgentMiddleware> = log_response();
    let _cleanup: Arc<dyn AgentMiddleware> = cleanup();
    let _custom_prompt: Arc<dyn AgentMiddleware> = custom_prompt();
}

// ---------------------------------------------------------------------------
// #[wrap_model_call] — compilation test
// ---------------------------------------------------------------------------

#[wrap_model_call]
async fn passthrough_model(
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    next.call(request).await
}

#[tokio::test]
async fn test_wrap_model_call_middleware() {
    // We cannot easily construct a real ModelCaller in a test without
    // spinning up a full model, but we can verify the macro compiles
    // correctly and produces a valid Arc<dyn AgentMiddleware>.
    let mw: Arc<dyn AgentMiddleware> = passthrough_model();
    // Verify the default methods still work (before_agent should no-op)
    let mut messages = vec![Message::human("hi")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 1);
}

// ---------------------------------------------------------------------------
// Default methods are no-ops
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_before_agent_default_methods_are_noop() {
    // A before_agent middleware should have no-op implementations for
    // all other AgentMiddleware methods.
    let mw: Arc<dyn AgentMiddleware> = setup();

    // before_model should be a no-op
    let mut req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    assert!(req.system_prompt.is_none());

    // after_agent should be a no-op
    let mut msgs = vec![];
    mw.after_agent(&mut msgs).await.unwrap();
    assert!(msgs.is_empty());
}

// ===========================================================================
// #[field] support tests
// ===========================================================================

// ---------------------------------------------------------------------------
// #[before_agent] with #[field]
// ---------------------------------------------------------------------------

#[before_agent]
async fn prefixed_setup(
    #[field] prefix: String,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    messages.push(Message::system(format!("{}: setup ran", prefix)));
    Ok(())
}

#[tokio::test]
async fn test_before_agent_with_field() {
    let mw: Arc<dyn AgentMiddleware> = prefixed_setup("BOT".to_string());
    let mut messages = vec![Message::human("hello")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "BOT: setup ran");
}

// ---------------------------------------------------------------------------
// #[before_model] with #[field]
// ---------------------------------------------------------------------------

#[before_model]
async fn inject_prompt(
    #[field] prompt: String,
    request: &mut ModelRequest,
) -> Result<(), SynapticError> {
    request.system_prompt = Some(prompt);
    Ok(())
}

#[tokio::test]
async fn test_before_model_with_field() {
    let mw: Arc<dyn AgentMiddleware> = inject_prompt("You are a pirate".to_string());
    let mut req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    assert_eq!(req.system_prompt.as_deref(), Some("You are a pirate"));
}

// ---------------------------------------------------------------------------
// #[after_model] with #[field]
// ---------------------------------------------------------------------------

#[after_model]
async fn tag_response(
    #[field] tag: String,
    _request: &ModelRequest,
    response: &mut ModelResponse,
) -> Result<(), SynapticError> {
    response.message = Message::ai(format!("[{}] {}", tag, response.message.content()));
    Ok(())
}

#[tokio::test]
async fn test_after_model_with_field() {
    let mw: Arc<dyn AgentMiddleware> = tag_response("v2".to_string());
    let req = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    let mut resp = ModelResponse {
        message: Message::ai("hi"),
        usage: None,
    };
    mw.after_model(&req, &mut resp).await.unwrap();
    assert_eq!(resp.message.content(), "[v2] hi");
}

// ---------------------------------------------------------------------------
// #[after_agent] with #[field]
// ---------------------------------------------------------------------------

#[after_agent]
async fn append_footer(
    #[field] footer: String,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    messages.push(Message::system(footer));
    Ok(())
}

#[tokio::test]
async fn test_after_agent_with_field() {
    let mw: Arc<dyn AgentMiddleware> = append_footer("-- end --".to_string());
    let mut messages = vec![Message::ai("done")];
    mw.after_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "-- end --");
}

// ---------------------------------------------------------------------------
// #[dynamic_prompt] with #[field]
// ---------------------------------------------------------------------------

#[dynamic_prompt]
fn branded_prompt(#[field] brand: String, messages: &[Message]) -> String {
    format!("[{}] You have {} messages", brand, messages.len())
}

#[tokio::test]
async fn test_dynamic_prompt_with_field() {
    let mw: Arc<dyn AgentMiddleware> = branded_prompt("Acme".to_string());
    let mut req = ModelRequest {
        messages: vec![Message::human("hi")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    mw.before_model(&mut req).await.unwrap();
    assert_eq!(
        req.system_prompt.as_deref(),
        Some("[Acme] You have 1 messages")
    );
}

// ---------------------------------------------------------------------------
// Multiple #[field] params
// ---------------------------------------------------------------------------

#[before_agent]
async fn multi_field_setup(
    #[field] prefix: String,
    #[field] max_messages: usize,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    if messages.len() < max_messages {
        messages.push(Message::system(format!("{}: initialized", prefix)));
    }
    Ok(())
}

#[tokio::test]
async fn test_multiple_fields() {
    let mw: Arc<dyn AgentMiddleware> =
        multi_field_setup("SYS".to_string(), 5);
    let mut messages = vec![Message::human("hello")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "SYS: initialized");

    // With max_messages = 1, no message should be added
    let mw2: Arc<dyn AgentMiddleware> =
        multi_field_setup("SYS".to_string(), 1);
    let mut messages2 = vec![Message::human("hello")];
    mw2.before_agent(&mut messages2).await.unwrap();
    assert_eq!(messages2.len(), 1); // not added because len() >= max_messages
}

// ---------------------------------------------------------------------------
// #[wrap_model_call] with #[field] — compilation test
// ---------------------------------------------------------------------------

#[wrap_model_call]
async fn retry_model(
    #[field] max_retries: usize,
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match next.call(request.clone()).await {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                let _ = attempt; // suppress unused warning
            }
        }
    }
    Err(last_err.unwrap())
}

#[tokio::test]
async fn test_wrap_model_call_with_field() {
    // Verify the macro compiles and produces a valid Arc<dyn AgentMiddleware>
    let mw: Arc<dyn AgentMiddleware> = retry_model(3);
    // Verify default methods still work
    let mut messages = vec![Message::human("hi")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 1);
}

// ---------------------------------------------------------------------------
// #[wrap_tool_call] with #[field] — compilation test
// ---------------------------------------------------------------------------

use synaptic_macros::wrap_tool_call;
use synaptic_middleware::ToolCaller;

#[wrap_tool_call]
async fn logged_tool_call(
    #[field] log_prefix: String,
    request: synaptic_middleware::ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<serde_json::Value, SynapticError> {
    let _ = format!("{}: calling {}", log_prefix, request.call.name);
    next.call(request).await
}

#[tokio::test]
async fn test_wrap_tool_call_with_field() {
    let mw: Arc<dyn AgentMiddleware> = logged_tool_call("LOG".to_string());
    // Verify default methods still work
    let mut messages = vec![Message::human("hi")];
    mw.before_agent(&mut messages).await.unwrap();
    assert_eq!(messages.len(), 1);
}
