use std::sync::Arc;

use synaptic_core::Message;
use synaptic_middleware::{AgentMiddleware, ModelRequest, ModelResponse};
use synaptic_secrets::{SecretMaskingMiddleware, SecretRegistry};

#[test]
fn register_mask() {
    let reg = SecretRegistry::new();
    reg.register("api_key", "sk-12345");

    let masked = reg.mask_output("My key is sk-12345");
    assert_eq!(masked, "My key is [REDACTED:api_key]");
    assert!(!masked.contains("sk-12345"));
}

#[test]
fn register_with_custom_mask() {
    let reg = SecretRegistry::new();
    reg.register_with_mask("token", "abc123", "***");

    let masked = reg.mask_output("Token: abc123");
    assert_eq!(masked, "Token: ***");
}

#[test]
fn inject_template() {
    let reg = SecretRegistry::new();
    reg.register("db_pass", "secret123");

    let result = reg
        .inject("Connect with password {{secret:db_pass}}")
        .unwrap();
    assert_eq!(result, "Connect with password secret123");
}

#[test]
fn inject_missing_secret_errors() {
    let reg = SecretRegistry::new();
    let result = reg.inject("Use {{secret:missing}}");
    assert!(result.is_err());
}

#[test]
fn mask_multiple() {
    let reg = SecretRegistry::new();
    reg.register("key1", "aaa");
    reg.register("key2", "bbb");

    let masked = reg.mask_output("Keys: aaa and bbb");
    assert!(masked.contains("[REDACTED:key1]"));
    assert!(masked.contains("[REDACTED:key2]"));
    assert!(!masked.contains("aaa"));
    assert!(!masked.contains("bbb"));
}

#[test]
fn remove_secret() {
    let reg = SecretRegistry::new();
    reg.register("key", "value");
    reg.remove("key");

    let masked = reg.mask_output("value");
    assert_eq!(masked, "value"); // Not masked after removal
}

#[tokio::test]
async fn middleware_redacts() {
    let reg = Arc::new(SecretRegistry::new());
    reg.register("api_key", "sk-secret");

    let mw = SecretMaskingMiddleware::new(reg);

    let request = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    };
    let mut response = ModelResponse {
        message: Message::ai("Your key is sk-secret"),
        usage: None,
    };

    mw.after_model(&request, &mut response).await.unwrap();
    assert!(response.message.content().contains("[REDACTED:api_key]"));
    assert!(!response.message.content().contains("sk-secret"));
}

#[tokio::test]
async fn middleware_injects() {
    let reg = Arc::new(SecretRegistry::new());
    reg.register("token", "my-secret-token");

    let mw = SecretMaskingMiddleware::new(reg);

    let mut request = ModelRequest {
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        system_prompt: Some("Use token: {{secret:token}}".to_string()),
    };

    mw.before_model(&mut request).await.unwrap();
    assert_eq!(request.system_prompt.unwrap(), "Use token: my-secret-token");
}
