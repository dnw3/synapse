use synaptic_core::{ChatModel, ModelProfile};
use synaptic_models::ScriptedChatModel;

#[test]
fn scripted_model_returns_none_profile() {
    let model = ScriptedChatModel::new(vec![]);
    // ScriptedChatModel uses default ChatModel::profile() which returns None
    assert!(model.profile().is_none());
}

#[test]
fn model_profile_fields() {
    let profile = ModelProfile {
        name: "gpt-4o".into(),
        provider: "openai".into(),
        supports_tool_calling: true,
        supports_structured_output: true,
        supports_streaming: true,
        max_input_tokens: Some(128_000),
        max_output_tokens: Some(4096),
    };
    assert_eq!(profile.name, "gpt-4o");
    assert_eq!(profile.provider, "openai");
    assert!(profile.supports_tool_calling);
    assert!(profile.supports_structured_output);
    assert!(profile.supports_streaming);
    assert_eq!(profile.max_input_tokens, Some(128_000));
    assert_eq!(profile.max_output_tokens, Some(4096));
}

#[test]
fn model_profile_serde_roundtrip() {
    let profile = ModelProfile {
        name: "claude-3".into(),
        provider: "anthropic".into(),
        supports_tool_calling: true,
        supports_structured_output: false,
        supports_streaming: true,
        max_input_tokens: Some(200_000),
        max_output_tokens: None,
    };
    let json = serde_json::to_string(&profile).unwrap();
    let deserialized: ModelProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "claude-3");
    assert_eq!(deserialized.provider, "anthropic");
    assert!(!deserialized.supports_structured_output);
    assert!(deserialized.max_output_tokens.is_none());
}

#[test]
fn model_profile_optional_token_limits() {
    let profile = ModelProfile {
        name: "local-model".into(),
        provider: "ollama".into(),
        supports_tool_calling: false,
        supports_structured_output: false,
        supports_streaming: true,
        max_input_tokens: None,
        max_output_tokens: None,
    };
    assert!(profile.max_input_tokens.is_none());
    assert!(profile.max_output_tokens.is_none());
}
