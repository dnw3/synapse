use std::sync::Arc;

use synaptic_core::{MemoryStore, Message, RunnableConfig, SynapticError};
use synaptic_memory::{InMemoryStore, RunnableWithMessageHistory};
use synaptic_runnables::Runnable;

/// A simple runnable that echoes the last human message content.
struct EchoRunnable;

#[async_trait::async_trait]
impl Runnable<Vec<Message>, String> for EchoRunnable {
    async fn invoke(
        &self,
        input: Vec<Message>,
        _config: &RunnableConfig,
    ) -> Result<String, SynapticError> {
        // Find the last human message and echo it
        let last_human = input
            .iter()
            .rev()
            .find(|m| m.is_human())
            .map(|m| format!("Echo: {}", m.content()))
            .unwrap_or_else(|| "No human message".to_string());
        Ok(last_human)
    }
}

#[tokio::test]
async fn runnable_with_message_history_loads_and_saves() {
    let store = Arc::new(InMemoryStore::new());
    let inner = EchoRunnable.boxed();
    let history = RunnableWithMessageHistory::new(inner, store.clone());

    let config = RunnableConfig::default().with_metadata(
        "session_id",
        serde_json::Value::String("test-session".to_string()),
    );

    // First invocation
    let output = history.invoke("hello".to_string(), &config).await.unwrap();
    assert_eq!(output, "Echo: hello");

    // Check that memory now has 2 messages (human + AI)
    let messages = store.load("test-session").await.unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].is_human());
    assert_eq!(messages[0].content(), "hello");
    assert!(messages[1].is_ai());
    assert_eq!(messages[1].content(), "Echo: hello");

    // Second invocation should see the full history
    let output2 = history.invoke("world".to_string(), &config).await.unwrap();
    assert_eq!(output2, "Echo: world");

    // Now memory should have 4 messages
    let messages = store.load("test-session").await.unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2].content(), "world");
    assert_eq!(messages[3].content(), "Echo: world");
}

#[tokio::test]
async fn session_isolation() {
    let store = Arc::new(InMemoryStore::new());
    let inner = EchoRunnable.boxed();
    let history = RunnableWithMessageHistory::new(inner, store.clone());

    let config_a = RunnableConfig::default().with_metadata(
        "session_id",
        serde_json::Value::String("session-a".to_string()),
    );
    let config_b = RunnableConfig::default().with_metadata(
        "session_id",
        serde_json::Value::String("session-b".to_string()),
    );

    history
        .invoke("hello-a".to_string(), &config_a)
        .await
        .unwrap();
    history
        .invoke("hello-b".to_string(), &config_b)
        .await
        .unwrap();

    let msgs_a = store.load("session-a").await.unwrap();
    let msgs_b = store.load("session-b").await.unwrap();
    assert_eq!(msgs_a.len(), 2); // human + ai
    assert_eq!(msgs_b.len(), 2);
    assert_eq!(msgs_a[0].content(), "hello-a");
    assert_eq!(msgs_b[0].content(), "hello-b");
}

#[tokio::test]
async fn message_roles_preserved() {
    let store = Arc::new(InMemoryStore::new());
    let inner = EchoRunnable.boxed();
    let history = RunnableWithMessageHistory::new(inner, store.clone());

    let config = RunnableConfig::default().with_metadata(
        "session_id",
        serde_json::Value::String("roles-test".to_string()),
    );

    history.invoke("test".to_string(), &config).await.unwrap();

    let msgs = store.load("roles-test").await.unwrap();
    assert!(msgs[0].is_human());
    assert!(msgs[1].is_ai());
}

#[tokio::test]
async fn system_message_in_history() {
    let store = Arc::new(InMemoryStore::new());

    // Pre-populate with a system message
    store
        .append("sys-test", Message::system("You are helpful"))
        .await
        .unwrap();

    let inner = EchoRunnable.boxed();
    let history = RunnableWithMessageHistory::new(inner, store.clone());

    let config = RunnableConfig::default().with_metadata(
        "session_id",
        serde_json::Value::String("sys-test".to_string()),
    );

    history.invoke("hi".to_string(), &config).await.unwrap();

    let msgs = store.load("sys-test").await.unwrap();
    assert_eq!(msgs.len(), 3); // system + human + ai
    assert!(msgs[0].is_system());
    assert!(msgs[1].is_human());
    assert!(msgs[2].is_ai());
}
