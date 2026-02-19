use std::sync::Arc;
use synaptic_deep::backend::{Backend, StateBackend};
use synaptic_deep::middleware::memory::DeepMemoryMiddleware;
use synaptic_middleware::{AgentMiddleware, ModelRequest};
use synaptic_core::Message;

fn empty_request() -> ModelRequest {
    ModelRequest {
        messages: vec![Message::human("hello")],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    }
}

#[tokio::test]
async fn loads_memory_file() {
    let backend = Arc::new(StateBackend::new());
    backend
        .write_file("AGENTS.md", "# Memory\n- Always use Rust.")
        .await
        .unwrap();

    let mw = DeepMemoryMiddleware::new(backend, "AGENTS.md".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();

    let prompt = request.system_prompt.unwrap();
    assert!(prompt.contains("<agent_memory>"));
    assert!(prompt.contains("Always use Rust"));
    assert!(prompt.contains("</agent_memory>"));
}

#[tokio::test]
async fn missing_memory_file_no_error() {
    let backend = Arc::new(StateBackend::new());
    let mw = DeepMemoryMiddleware::new(backend, "AGENTS.md".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();
    assert!(request.system_prompt.is_none());
}

#[tokio::test]
async fn appends_to_existing_prompt() {
    let backend = Arc::new(StateBackend::new());
    backend.write_file("mem.md", "Remember this.").await.unwrap();

    let mw = DeepMemoryMiddleware::new(backend, "mem.md".to_string());
    let mut request = empty_request();
    request.system_prompt = Some("You are helpful.".to_string());
    mw.before_model(&mut request).await.unwrap();

    let prompt = request.system_prompt.unwrap();
    assert!(prompt.starts_with("You are helpful."));
    assert!(prompt.contains("Remember this."));
}

#[tokio::test]
async fn empty_memory_file_no_injection() {
    let backend = Arc::new(StateBackend::new());
    backend.write_file("AGENTS.md", "").await.unwrap();

    let mw = DeepMemoryMiddleware::new(backend, "AGENTS.md".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();
    assert!(request.system_prompt.is_none());
}
