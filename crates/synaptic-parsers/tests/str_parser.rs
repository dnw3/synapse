use synaptic_core::{Message, RunnableConfig};
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn extracts_content_from_ai_message() {
    let parser = StrOutputParser;
    let config = RunnableConfig::default();
    let msg = Message::ai("Hello, world!");
    let result = parser.invoke(msg, &config).await.unwrap();
    assert_eq!(result, "Hello, world!");
}

#[tokio::test]
async fn extracts_content_from_human_message() {
    let parser = StrOutputParser;
    let config = RunnableConfig::default();
    let msg = Message::human("User input");
    let result = parser.invoke(msg, &config).await.unwrap();
    assert_eq!(result, "User input");
}

#[tokio::test]
async fn extracts_content_from_system_message() {
    let parser = StrOutputParser;
    let config = RunnableConfig::default();
    let msg = Message::system("System prompt");
    let result = parser.invoke(msg, &config).await.unwrap();
    assert_eq!(result, "System prompt");
}
