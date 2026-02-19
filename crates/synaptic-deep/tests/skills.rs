use std::sync::Arc;
use synaptic_deep::backend::{Backend, StateBackend};
use synaptic_deep::middleware::skills::SkillsMiddleware;
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
async fn no_skills_no_injection() {
    let backend = Arc::new(StateBackend::new());
    let mw = SkillsMiddleware::new(backend, ".skills".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();
    assert!(request.system_prompt.is_none());
}

#[tokio::test]
async fn discovers_skills_from_frontmatter() {
    let backend = Arc::new(StateBackend::new());
    // Create a skill directory structure
    backend
        .write_file(
            ".skills/search/SKILL.md",
            "---\nname: search\ndescription: Search the web\n---\n# Search\nDetails here.",
        )
        .await
        .unwrap();
    backend
        .write_file(
            ".skills/code/SKILL.md",
            "---\nname: code-review\ndescription: Review code for issues\n---\n# Code Review",
        )
        .await
        .unwrap();

    let mw = SkillsMiddleware::new(backend, ".skills".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();

    let prompt = request.system_prompt.unwrap();
    assert!(prompt.contains("<available_skills>"));
    assert!(prompt.contains("search"));
    assert!(prompt.contains("code-review"));
    assert!(prompt.contains("Search the web"));
}

#[tokio::test]
async fn appends_to_existing_system_prompt() {
    let backend = Arc::new(StateBackend::new());
    backend
        .write_file(
            ".skills/tool/SKILL.md",
            "---\nname: my-tool\ndescription: A tool\n---\n",
        )
        .await
        .unwrap();

    let mw = SkillsMiddleware::new(backend, ".skills".to_string());
    let mut request = empty_request();
    request.system_prompt = Some("You are helpful.".to_string());
    mw.before_model(&mut request).await.unwrap();

    let prompt = request.system_prompt.unwrap();
    assert!(prompt.starts_with("You are helpful."));
    assert!(prompt.contains("my-tool"));
}

#[tokio::test]
async fn invalid_frontmatter_skipped() {
    let backend = Arc::new(StateBackend::new());
    // No frontmatter
    backend
        .write_file(".skills/bad/SKILL.md", "# Just a header\nNo frontmatter here.")
        .await
        .unwrap();

    let mw = SkillsMiddleware::new(backend, ".skills".to_string());
    let mut request = empty_request();
    mw.before_model(&mut request).await.unwrap();
    assert!(request.system_prompt.is_none());
}
