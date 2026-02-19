use serde_json::Value;
use std::sync::Arc;
use synaptic_deep::backend::{Backend, StateBackend};
use synaptic_deep::middleware::filesystem::FilesystemMiddleware;
use synaptic_middleware::{AgentMiddleware, ToolCallRequest, ToolCaller};
use synaptic_core::{SynapticError, ToolCall};
use async_trait::async_trait;

/// A mock ToolCaller that returns a fixed result.
struct MockToolCaller {
    result: Value,
}

#[async_trait]
impl ToolCaller for MockToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        Ok(self.result.clone())
    }
}

fn make_request(id: &str) -> ToolCallRequest {
    ToolCallRequest {
        call: ToolCall {
            id: id.to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({}),
        },
    }
}

#[tokio::test]
async fn small_result_passes_through() {
    let backend = Arc::new(StateBackend::new());
    let mw = FilesystemMiddleware::new(backend, 20_000); // 80K char threshold

    let small_result = Value::String("small content".to_string());
    let caller = MockToolCaller { result: small_result.clone() };

    let result = mw.wrap_tool_call(make_request("tc_1"), &caller).await.unwrap();
    assert_eq!(result, small_result);
}

#[tokio::test]
async fn large_result_gets_evicted() {
    let backend = Arc::new(StateBackend::new());
    // Low threshold: 10 tokens = 40 chars
    let mw = FilesystemMiddleware::new(backend.clone(), 10);

    // Create a result larger than 40 chars with >10 lines
    let large = (0..20).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let caller = MockToolCaller { result: Value::String(large.clone()) };

    let result = mw.wrap_tool_call(make_request("tc_evict"), &caller).await.unwrap();
    let result_str = result.as_str().unwrap();

    // Result should be a preview, not the full content
    assert!(result_str.contains("lines omitted"));
    assert!(result_str.contains(".evicted/tc_evict.txt"));

    // Full result should be saved in backend
    let saved = backend.read_file(".evicted/tc_evict.txt", 0, 10000).await.unwrap();
    assert_eq!(saved, large);
}

#[tokio::test]
async fn threshold_boundary() {
    let backend = Arc::new(StateBackend::new());
    // Threshold: 100 tokens = 400 chars
    let mw = FilesystemMiddleware::new(backend, 100);

    // Just under threshold (399 chars)
    let under = "x".repeat(399);
    let caller = MockToolCaller { result: Value::String(under.clone()) };
    let result = mw.wrap_tool_call(make_request("tc_under"), &caller).await.unwrap();
    assert_eq!(result.as_str().unwrap(), &under);

    // Over threshold (>400 chars, needs >10 lines for preview format)
    let over = (0..100).map(|i| format!("this is line number {}", i)).collect::<Vec<_>>().join("\n");
    let caller = MockToolCaller { result: Value::String(over) };
    let result = mw.wrap_tool_call(make_request("tc_over"), &caller).await.unwrap();
    assert!(result.as_str().unwrap().contains("lines omitted"));
}
