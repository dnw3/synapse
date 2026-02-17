use std::collections::HashMap;

use serde_json::{json, Value};
use synaptic_core::{Message, RunnableConfig};
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate, PromptTemplate};
use synaptic_runnables::Runnable;

#[test]
fn chat_template_renders_system_and_human() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new("You are a {{ role }}.")),
        MessageTemplate::Human(PromptTemplate::new("{{ input }}")),
    ]);

    let values: HashMap<String, Value> = HashMap::from([
        ("role".to_string(), json!("helpful assistant")),
        ("input".to_string(), json!("Hello!")),
    ]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], Message::system("You are a helpful assistant."));
    assert_eq!(messages[1], Message::human("Hello!"));
}

#[test]
fn chat_template_with_placeholder() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new("You are helpful.")),
        MessageTemplate::Placeholder("history".to_string()),
        MessageTemplate::Human(PromptTemplate::new("{{ input }}")),
    ]);

    let history = json!([
        {"role": "human", "content": "Hi"},
        {"role": "assistant", "content": "Hello!"}
    ]);

    let values: HashMap<String, Value> = HashMap::from([
        ("history".to_string(), history),
        ("input".to_string(), json!("How are you?")),
    ]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 4);
    assert!(messages[0].is_system());
    assert!(messages[1].is_human());
    assert_eq!(messages[1].content(), "Hi");
    assert!(messages[2].is_ai());
    assert_eq!(messages[2].content(), "Hello!");
    assert!(messages[3].is_human());
    assert_eq!(messages[3].content(), "How are you?");
}

#[test]
fn chat_template_missing_variable_error() {
    let prompt = ChatPromptTemplate::from_messages(vec![MessageTemplate::Human(
        PromptTemplate::new("{{ missing }}"),
    )]);

    let values: HashMap<String, Value> = HashMap::new();
    let err = prompt.format(&values).unwrap_err();
    assert!(err.to_string().contains("missing"));
}

#[test]
fn chat_template_missing_placeholder_error() {
    let prompt = ChatPromptTemplate::from_messages(vec![MessageTemplate::Placeholder(
        "history".to_string(),
    )]);

    let values: HashMap<String, Value> = HashMap::new();
    let err = prompt.format(&values).unwrap_err();
    assert!(err.to_string().contains("history"));
}

#[test]
fn chat_template_with_ai_message() {
    let prompt = ChatPromptTemplate::from_messages(vec![MessageTemplate::AI(PromptTemplate::new(
        "I will help with {{ task }}.",
    ))]);

    let values: HashMap<String, Value> = HashMap::from([("task".to_string(), json!("coding"))]);

    let messages = prompt.format(&values).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].is_ai());
    assert_eq!(messages[0].content(), "I will help with coding.");
}

#[tokio::test]
async fn chat_template_as_runnable() {
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new("You are a bot.")),
        MessageTemplate::Human(PromptTemplate::new("{{ question }}")),
    ]);

    let values: HashMap<String, Value> =
        HashMap::from([("question".to_string(), json!("What is Rust?"))]);

    let config = RunnableConfig::default();
    let messages = prompt.invoke(values, &config).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content(), "What is Rust?");
}
