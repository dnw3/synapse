use std::collections::HashMap;

use serde_json::{json, Value};
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate, PromptTemplate};

#[test]
fn placeholder_with_empty_array() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new("System.")),
        MessageTemplate::Placeholder("history".to_string()),
        MessageTemplate::Human(PromptTemplate::new("{{ input }}")),
    ]);

    let values: HashMap<String, Value> = HashMap::from([
        ("history".to_string(), json!([])),
        ("input".to_string(), json!("hi")),
    ]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 2); // system + human (empty placeholder)
    assert!(messages[0].is_system());
    assert!(messages[1].is_human());
    assert_eq!(messages[1].content(), "hi");
}

#[test]
fn placeholder_with_multiple_message_types() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::Placeholder("msgs".to_string()),
    ]);

    let values: HashMap<String, Value> = HashMap::from([(
        "msgs".to_string(),
        json!([
            {"role": "system", "content": "Be helpful"},
            {"role": "human", "content": "Hello"},
            {"role": "assistant", "content": "Hi!"},
            {"role": "tool", "content": "result", "tool_call_id": "c1"}
        ]),
    )]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 4);
    assert!(messages[0].is_system());
    assert!(messages[1].is_human());
    assert!(messages[2].is_ai());
    assert!(messages[3].is_tool());
}

#[test]
fn multiple_placeholders_in_sequence() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::Placeholder("context".to_string()),
        MessageTemplate::Placeholder("history".to_string()),
        MessageTemplate::Human(PromptTemplate::new("{{ question }}")),
    ]);

    let values: HashMap<String, Value> = HashMap::from([
        ("context".to_string(), json!([{"role": "system", "content": "Context here"}])),
        ("history".to_string(), json!([{"role": "human", "content": "Previous q"}])),
        ("question".to_string(), json!("New question")),
    ]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].content(), "Context here");
    assert_eq!(messages[1].content(), "Previous q");
    assert_eq!(messages[2].content(), "New question");
}

#[test]
fn placeholder_missing_key_returns_error() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::Placeholder("missing_key".to_string()),
    ]);

    let values: HashMap<String, Value> = HashMap::new();
    let err = prompt.format(&values).unwrap_err();
    assert!(err.to_string().contains("missing_key"));
}

#[test]
fn chat_template_format_preserves_order() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new("First")),
        MessageTemplate::Human(PromptTemplate::new("Second")),
        MessageTemplate::AI(PromptTemplate::new("Third")),
    ]);

    let values: HashMap<String, Value> = HashMap::new();
    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages[0].content(), "First");
    assert_eq!(messages[1].content(), "Second");
    assert_eq!(messages[2].content(), "Third");
}
