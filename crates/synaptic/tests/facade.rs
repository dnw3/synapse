use synaptic::core::{Message, RunnableConfig, SynapseError};

#[test]
fn facade_reexports_core_types() {
    let msg = Message::human("hello");
    assert_eq!(msg.content(), "hello");

    let config = RunnableConfig::default();
    assert!(config.tags.is_empty());

    // Verify SynapseError is accessible through the facade.
    let err = SynapseError::Validation("test".into());
    assert!(matches!(err, SynapseError::Validation(_)));
}

#[cfg(feature = "retrieval")]
#[test]
fn facade_reexports_retrieval_types() {
    use synaptic::retrieval::Document;

    let doc = Document::new("1", "test content");
    assert_eq!(doc.id, "1");
}

#[cfg(feature = "models")]
#[test]
fn facade_reexports_models() {
    // ScriptedChatModel is accessible
    let _model = synaptic::models::ScriptedChatModel::new(vec![]);
}

#[cfg(feature = "runnables")]
#[tokio::test]
async fn facade_reexports_runnables() {
    use synaptic::core::RunnableConfig;
    use synaptic::runnables::{Runnable, RunnableLambda, RunnablePassthrough};

    let config = RunnableConfig::default();
    let pass: String = RunnablePassthrough
        .invoke("hello".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(pass, "hello");

    let upper = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let result = upper.invoke("test".to_string(), &config).await.unwrap();
    assert_eq!(result, "TEST");
}

#[cfg(feature = "prompts")]
#[test]
fn facade_reexports_prompts() {
    use synaptic::prompts::{ChatPromptTemplate, MessageTemplate, PromptTemplate};
    let _template = ChatPromptTemplate::from_messages(vec![MessageTemplate::Human(
        PromptTemplate::new("{{ input }}"),
    )]);
}

#[cfg(feature = "parsers")]
#[tokio::test]
async fn facade_reexports_parsers() {
    use synaptic::core::{Message, RunnableConfig};
    use synaptic::parsers::StrOutputParser;
    use synaptic::runnables::Runnable;

    let config = RunnableConfig::default();
    let parser = StrOutputParser;
    let result = parser.invoke(Message::ai("hello"), &config).await.unwrap();
    assert_eq!(result, "hello");
}

#[cfg(feature = "tools")]
#[test]
fn facade_reexports_tools() {
    use synaptic::tools::ToolRegistry;
    let registry = ToolRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[cfg(feature = "memory")]
#[tokio::test]
async fn facade_reexports_memory() {
    use synaptic::core::{MemoryStore, Message};
    use synaptic::memory::InMemoryStore;

    let store = InMemoryStore::new();
    store.append("s1", Message::human("hi")).await.unwrap();
    let msgs = store.load("s1").await.unwrap();
    assert_eq!(msgs.len(), 1);
}

#[cfg(feature = "callbacks")]
#[tokio::test]
async fn facade_reexports_callbacks() {
    use synaptic::callbacks::RecordingCallback;
    use synaptic::core::{CallbackHandler, RunEvent};

    let cb = RecordingCallback::new();
    cb.on_event(RunEvent::RunStarted {
        run_id: "r1".to_string(),
        session_id: "s1".to_string(),
    })
    .await
    .unwrap();
    assert_eq!(cb.events().await.len(), 1);
}

#[cfg(feature = "graph")]
#[test]
fn facade_reexports_graph() {
    use synaptic::graph::MessageState;
    // Verify types are accessible
    let _state = MessageState { messages: vec![] };
}

#[cfg(feature = "cache")]
#[tokio::test]
async fn facade_reexports_cache() {
    use synaptic::cache::{InMemoryCache, LlmCache};

    let cache = InMemoryCache::new();
    let result = cache.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[cfg(feature = "eval")]
#[test]
fn facade_reexports_eval() {
    use synaptic::eval::{Dataset, ExactMatchEvaluator};
    let _eval = ExactMatchEvaluator::new();
    let _ds = Dataset::from_pairs(vec![("a", "b")]);
}
