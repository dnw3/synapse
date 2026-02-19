//! Integration tests proving the middleware macros can build real
//! stateful middleware (equivalent to ToolRetryMiddleware and
//! ModelFallbackMiddleware).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic_macros::{wrap_model_call, wrap_tool_call};
use synaptic_middleware::{
    AgentMiddleware, BaseChatModelCaller, MiddlewareChain, ModelCaller, ModelRequest, ModelResponse,
    ToolCallRequest, ToolCaller,
};

// ===========================================================================
// Macro-based ToolRetryMiddleware equivalent
// ===========================================================================

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

// ===========================================================================
// Macro-based ModelFallbackMiddleware equivalent
// ===========================================================================

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
                match caller.call(request.clone()).await {
                    Ok(resp) => return Ok(resp),
                    Err(_) => continue,
                }
            }
            Err(primary_err)
        }
    }
}

// ===========================================================================
// Test helpers
// ===========================================================================

struct FlakeyToolCaller {
    fail_count: AtomicUsize,
    fail_until: usize,
}

impl FlakeyToolCaller {
    fn new(fail_until: usize) -> Self {
        Self {
            fail_count: AtomicUsize::new(0),
            fail_until,
        }
    }

    fn call_count(&self) -> usize {
        self.fail_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ToolCaller for FlakeyToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
        if count < self.fail_until {
            Err(SynapticError::Tool(format!("fail #{}", count)))
        } else {
            Ok(json!({"ok": true}))
        }
    }
}

struct AlwaysFailToolCaller;

#[async_trait]
impl ToolCaller for AlwaysFailToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        Err(SynapticError::Tool("always fail".into()))
    }
}

struct FailingModel;

#[async_trait]
impl ChatModel for FailingModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Err(SynapticError::Model("primary down".into()))
    }
}

struct EchoModel {
    reply: String,
}

impl EchoModel {
    fn new(reply: &str) -> Self {
        Self {
            reply: reply.to_string(),
        }
    }
}

#[async_trait]
impl ChatModel for EchoModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai(&self.reply),
            usage: None,
        })
    }
}

fn make_tool_request(name: &str) -> ToolCallRequest {
    ToolCallRequest {
        call: synaptic_core::ToolCall {
            id: "call-1".into(),
            name: name.into(),
            arguments: json!({}),
        },
    }
}

// ===========================================================================
// Tool retry tests
// ===========================================================================

#[tokio::test]
async fn test_tool_retry_succeeds_after_failures() {
    let mw: Arc<dyn AgentMiddleware> = tool_retry(3, Duration::from_millis(1));
    let caller = FlakeyToolCaller::new(2); // fails twice, then succeeds

    let chain = MiddlewareChain::new(vec![mw]);
    let result = chain
        .call_tool(make_tool_request("test"), &caller)
        .await
        .unwrap();

    assert_eq!(result, json!({"ok": true}));
    assert_eq!(caller.call_count(), 3); // 2 failures + 1 success
}

#[tokio::test]
async fn test_tool_retry_exhausts_retries() {
    let mw: Arc<dyn AgentMiddleware> = tool_retry(2, Duration::from_millis(1));
    let caller = AlwaysFailToolCaller;

    let chain = MiddlewareChain::new(vec![mw]);
    let result = chain
        .call_tool(make_tool_request("test"), &caller)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("always fail"));
}

#[tokio::test]
async fn test_tool_retry_no_retry_on_success() {
    let mw: Arc<dyn AgentMiddleware> = tool_retry(3, Duration::from_millis(1));
    let caller = FlakeyToolCaller::new(0); // succeeds immediately

    let chain = MiddlewareChain::new(vec![mw]);
    let result = chain
        .call_tool(make_tool_request("test"), &caller)
        .await
        .unwrap();

    assert_eq!(result, json!({"ok": true}));
    assert_eq!(caller.call_count(), 1);
}

// ===========================================================================
// Model fallback tests
// ===========================================================================

#[tokio::test]
async fn test_model_fallback_uses_fallback_on_failure() {
    let primary = Arc::new(FailingModel) as Arc<dyn ChatModel>;
    let fallback = Arc::new(EchoModel::new("from fallback")) as Arc<dyn ChatModel>;

    let mw: Arc<dyn AgentMiddleware> = model_fallback(vec![fallback]);
    let base = BaseChatModelCaller::new(primary);

    let chain = MiddlewareChain::new(vec![mw]);
    let req = ModelRequest {
        messages: vec![Message::human("hello")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };

    let resp = chain.call_model(req, &base).await.unwrap();
    assert_eq!(resp.message.content(), "from fallback");
}

#[tokio::test]
async fn test_model_fallback_all_fail() {
    let primary = Arc::new(FailingModel) as Arc<dyn ChatModel>;
    let fallback = Arc::new(FailingModel) as Arc<dyn ChatModel>;

    let mw: Arc<dyn AgentMiddleware> = model_fallback(vec![fallback]);
    let base = BaseChatModelCaller::new(primary);

    let chain = MiddlewareChain::new(vec![mw]);
    let req = ModelRequest {
        messages: vec![Message::human("hello")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };

    let result = chain.call_model(req, &base).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("primary down"));
}

#[tokio::test]
async fn test_model_fallback_primary_succeeds() {
    let primary = Arc::new(EchoModel::new("from primary")) as Arc<dyn ChatModel>;
    let fallback = Arc::new(EchoModel::new("from fallback")) as Arc<dyn ChatModel>;

    let mw: Arc<dyn AgentMiddleware> = model_fallback(vec![fallback]);
    let base = BaseChatModelCaller::new(primary);

    let chain = MiddlewareChain::new(vec![mw]);
    let req = ModelRequest {
        messages: vec![Message::human("hello")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };

    let resp = chain.call_model(req, &base).await.unwrap();
    assert_eq!(resp.message.content(), "from primary");
}
