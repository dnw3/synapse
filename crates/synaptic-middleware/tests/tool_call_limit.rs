use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, ToolCall};
use synaptic_middleware::{AgentMiddleware, ToolCallLimitMiddleware, ToolCallRequest, ToolCaller};

// ---------------------------------------------------------------------------
// Mock ToolCaller
// ---------------------------------------------------------------------------

struct MockToolCaller;

#[async_trait]
impl ToolCaller for MockToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        Ok(json!("tool result"))
    }
}

fn make_request(name: &str) -> ToolCallRequest {
    ToolCallRequest {
        call: ToolCall {
            id: "tc-1".to_string(),
            name: name.to_string(),
            arguments: json!({}),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn allows_calls_within_limit() {
    let mw = ToolCallLimitMiddleware::new(3);
    let next = MockToolCaller;

    for _ in 0..3 {
        let result = mw.wrap_tool_call(make_request("search"), &next).await;
        assert!(result.is_ok(), "call within limit should succeed");
    }
}

#[tokio::test]
async fn blocks_call_at_limit() {
    let mw = ToolCallLimitMiddleware::new(2);
    let next = MockToolCaller;

    // Two calls succeed (indices 0, 1)
    assert!(mw.wrap_tool_call(make_request("a"), &next).await.is_ok());
    assert!(mw.wrap_tool_call(make_request("b"), &next).await.is_ok());

    // Third call should fail (index 2, which is >= max_calls 2)
    let result = mw.wrap_tool_call(make_request("c"), &next).await;
    assert!(result.is_err(), "call at limit should fail");
}

#[tokio::test]
async fn call_count_tracks_correctly() {
    let mw = ToolCallLimitMiddleware::new(10);
    let next = MockToolCaller;

    assert_eq!(mw.call_count(), 0);

    mw.wrap_tool_call(make_request("a"), &next).await.unwrap();
    assert_eq!(mw.call_count(), 1);

    mw.wrap_tool_call(make_request("b"), &next).await.unwrap();
    assert_eq!(mw.call_count(), 2);

    mw.wrap_tool_call(make_request("c"), &next).await.unwrap();
    assert_eq!(mw.call_count(), 3);
}

#[tokio::test]
async fn call_count_increments_even_on_blocked_calls() {
    let mw = ToolCallLimitMiddleware::new(1);
    let next = MockToolCaller;

    // First call succeeds
    mw.wrap_tool_call(make_request("a"), &next).await.unwrap();
    assert_eq!(mw.call_count(), 1);

    // Second call fails but counter still increments (fetch_add before check)
    let _ = mw.wrap_tool_call(make_request("b"), &next).await;
    assert_eq!(mw.call_count(), 2);
}

#[tokio::test]
async fn reset_clears_count_and_allows_new_calls() {
    let mw = ToolCallLimitMiddleware::new(1);
    let next = MockToolCaller;

    mw.wrap_tool_call(make_request("a"), &next).await.unwrap();
    assert_eq!(mw.call_count(), 1);

    // Next call would fail
    assert!(mw.wrap_tool_call(make_request("b"), &next).await.is_err());

    // Reset and try again
    mw.reset();
    assert_eq!(mw.call_count(), 0);
    assert!(mw.wrap_tool_call(make_request("c"), &next).await.is_ok());
}

#[tokio::test]
async fn limit_of_one_allows_exactly_one_call() {
    let mw = ToolCallLimitMiddleware::new(1);
    let next = MockToolCaller;

    let first = mw.wrap_tool_call(make_request("x"), &next).await;
    assert!(first.is_ok());

    let second = mw.wrap_tool_call(make_request("y"), &next).await;
    assert!(second.is_err());
}

#[tokio::test]
async fn error_is_max_steps_exceeded_variant() {
    let mw = ToolCallLimitMiddleware::new(0);
    let next = MockToolCaller;

    let result = mw.wrap_tool_call(make_request("x"), &next).await;
    match result {
        Err(SynapticError::MaxStepsExceeded { max_steps }) => {
            assert_eq!(max_steps, 0);
        }
        other => panic!("expected MaxStepsExceeded, got {:?}", other),
    }
}
