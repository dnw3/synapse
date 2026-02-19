use async_trait::async_trait;
use futures::StreamExt;
use synaptic::core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, RunnableConfig,
    SynapticError,
};
use synaptic::runnables::{Runnable, RunnableLambda};

/// A model that streams its response word by word.
struct StreamingModel {
    words: Vec<String>,
}

impl StreamingModel {
    fn new(sentence: &str) -> Self {
        Self {
            words: sentence.split_whitespace().map(String::from).collect(),
        }
    }
}

#[async_trait]
impl ChatModel for StreamingModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai(self.words.join(" ")),
            usage: None,
        })
    }

    fn stream_chat(&self, _request: ChatRequest) -> ChatStream<'_> {
        let words = self.words.clone();
        Box::pin(async_stream::stream! {
            for (i, word) in words.iter().enumerate() {
                let content = if i == 0 {
                    word.clone()
                } else {
                    format!(" {word}")
                };
                yield Ok(AIMessageChunk {
                    content,
                    ..Default::default()
                });
            }
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let config = RunnableConfig::default();
    let model = StreamingModel::new("Rust is a fast and safe language");

    // --- Streaming from model ---
    println!("=== Streaming from Model ===");
    let request = ChatRequest::new(vec![Message::human("Tell me about Rust")]);
    let mut stream = model.stream_chat(request);
    let mut merged = AIMessageChunk::default();
    print!("Chunks: ");
    while let Some(result) = stream.next().await {
        let chunk = result?;
        print!("[{}]", chunk.content);
        merged += chunk;
    }
    println!();
    println!("Merged: {}", merged.content);

    // --- Convert merged chunk to Message ---
    println!("\n=== Chunk to Message ===");
    let message = merged.into_message();
    println!(
        "Message role: {}, content: {}",
        message.role(),
        message.content()
    );

    // --- Streaming through a Runnable pipeline ---
    println!("\n=== Streaming through Pipeline ===");
    let step = RunnableLambda::new(|s: String| async move { Ok(format!(">> {s}")) });
    let boxed = step.boxed();
    let mut stream = boxed.stream("hello streaming".to_string(), &config);
    while let Some(result) = stream.next().await {
        let output = result?;
        println!("Stream item: {output}");
    }

    println!("\nStreaming demo completed successfully!");
    Ok(())
}
