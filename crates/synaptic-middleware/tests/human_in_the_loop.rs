use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, ToolCall};
use synaptic_middleware::{
    AgentMiddleware, ApprovalCallback, HumanInTheLoopMiddleware, ToolCallRequest, ToolCaller,
};

// ---------------------------------------------------------------------------
// Approval callback implementations
// ---------------------------------------------------------------------------

struct AlwaysApprove;

#[async_trait]
impl ApprovalCallback for AlwaysApprove {
    async fn approve(&self, _tool_name: &str, _arguments: &Value) -> Result<bool, SynapticError> {
        Ok(true)
    }
}

struct AlwaysReject;

#[async_trait]
impl ApprovalCallback for AlwaysReject {
    async fn approve(&self, _tool_name: &str, _arguments: &Value) -> Result<bool, SynapticError> {
        Ok(false)
    }
}

/// Records the tool name and arguments it was called with, then approves.
struct RecordingCallback {
    recorded: tokio::sync::Mutex<Vec<(String, Value)>>,
}

impl RecordingCallback {
    fn new() -> Self {
        Self {
            recorded: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ApprovalCallback for RecordingCallback {
    async fn approve(&self, tool_name: &str, arguments: &Value) -> Result<bool, SynapticError> {
        self.recorded
            .lock()
            .await
            .push((tool_name.to_string(), arguments.clone()));
        Ok(true)
    }
}

/// Returns an error from the callback.
struct FailingCallback;

#[async_trait]
impl ApprovalCallback for FailingCallback {
    async fn approve(&self, _tool_name: &str, _arguments: &Value) -> Result<bool, SynapticError> {
        Err(SynapticError::Callback("approval system unavailable".into()))
    }
}

// ---------------------------------------------------------------------------
// Mock ToolCaller
// ---------------------------------------------------------------------------

struct MockToolCaller;

#[async_trait]
impl ToolCaller for MockToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        Ok(json!("executed"))
    }
}

fn make_request(name: &str, args: Value) -> ToolCallRequest {
    ToolCallRequest {
        call: ToolCall {
            id: "tc-1".to_string(),
            name: name.to_string(),
            arguments: args,
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn approved_call_passes_through() {
    let mw = HumanInTheLoopMiddleware::new(Arc::new(AlwaysApprove));
    let next = MockToolCaller;

    let result = mw
        .wrap_tool_call(make_request("search", json!({})), &next)
        .await
        .unwrap();
    assert_eq!(result, json!("executed"));
}

#[tokio::test]
async fn rejected_call_returns_rejection_message() {
    let mw = HumanInTheLoopMiddleware::new(Arc::new(AlwaysReject));
    let next = MockToolCaller;

    let result = mw
        .wrap_tool_call(make_request("delete_all", json!({})), &next)
        .await
        .unwrap();

    // Rejection returns an Ok(Value::String) with a rejection message,
    // not an Err. The message is fed back to the model.
    let msg = result.as_str().unwrap();
    assert!(
        msg.contains("rejected"),
        "rejection message should mention 'rejected', got: {}",
        msg
    );
    assert!(
        msg.contains("delete_all"),
        "rejection message should contain tool name, got: {}",
        msg
    );
}

#[tokio::test]
async fn for_tools_guards_only_listed_tools() {
    let mw = HumanInTheLoopMiddleware::for_tools(
        Arc::new(AlwaysReject),
        vec!["dangerous_tool".to_string()],
    );
    let next = MockToolCaller;

    // The listed tool should be rejected
    let result = mw
        .wrap_tool_call(make_request("dangerous_tool", json!({})), &next)
        .await
        .unwrap();
    let msg = result.as_str().unwrap();
    assert!(msg.contains("rejected"));
}

#[tokio::test]
async fn for_tools_allows_unlisted_tools_without_approval() {
    let mw = HumanInTheLoopMiddleware::for_tools(
        Arc::new(AlwaysReject),
        vec!["dangerous_tool".to_string()],
    );
    let next = MockToolCaller;

    // An unlisted tool should bypass approval entirely and execute
    let result = mw
        .wrap_tool_call(make_request("safe_tool", json!({})), &next)
        .await
        .unwrap();
    assert_eq!(result, json!("executed"));
}

#[tokio::test]
async fn approval_callback_receives_correct_tool_name_and_args() {
    let recorder = Arc::new(RecordingCallback::new());
    let mw = HumanInTheLoopMiddleware::new(recorder.clone());
    let next = MockToolCaller;

    let args = json!({"query": "test", "limit": 10});
    mw.wrap_tool_call(make_request("search", args.clone()), &next)
        .await
        .unwrap();

    let recorded = recorder.recorded.lock().await;
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, "search");
    assert_eq!(recorded[0].1, args);
}

#[tokio::test]
async fn callback_error_propagates_to_caller() {
    let mw = HumanInTheLoopMiddleware::new(Arc::new(FailingCallback));
    let next = MockToolCaller;

    let result = mw
        .wrap_tool_call(make_request("anything", json!({})), &next)
        .await;
    assert!(result.is_err(), "callback error should propagate");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("approval system unavailable"),
        "error should contain callback error message, got: {}",
        err_msg
    );
}
