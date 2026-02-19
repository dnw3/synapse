use std::collections::HashMap;

use serde_json::Value;
use synaptic::core::{ChatResponse, Message, RunnableConfig, SynapticError};
use synaptic::models::ScriptedChatModel;
use synaptic::parsers::StrOutputParser;
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate, PromptTemplate};
use synaptic::runnables::{Runnable, RunnableLambda};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let config = RunnableConfig::default();

    // --- Build prompt template ---
    println!("=== Prompt Template ===");
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new(
            "You are a helpful assistant that speaks {{ language }}.",
        )),
        MessageTemplate::Human(PromptTemplate::new("{{ question }}")),
    ]);

    let mut values = HashMap::new();
    values.insert("language".to_string(), Value::String("English".to_string()));
    values.insert(
        "question".to_string(),
        Value::String("What is Rust?".to_string()),
    );
    let messages = prompt.invoke(values.clone(), &config).await?;
    for msg in &messages {
        println!("[{}] {}", msg.role(), msg.content());
    }

    // --- Scripted model ---
    println!("\n=== Scripted Model ===");
    let model = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            "Rust is a systems programming language focused on safety and performance.",
        ),
        usage: None,
    }]);

    // --- Full chain: prompt -> model -> parser ---
    println!("\n=== Full Chain: Prompt -> Model -> Parser ===");
    let model_clone = model.clone();
    let model_step = RunnableLambda::new(move |messages: Vec<Message>| {
        let m = model_clone.clone();
        async move {
            use synaptic::core::{ChatModel, ChatRequest};
            let request = ChatRequest::new(messages);
            let response = m.chat(request).await?;
            Ok(response.message)
        }
    });

    let chain = prompt.boxed() | model_step.boxed() | StrOutputParser.boxed();
    let result = chain.invoke(values, &config).await?;
    println!("Chain output: {result}");

    println!("\nPrompt-parser chain demo completed successfully!");
    Ok(())
}
