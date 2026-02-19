use std::sync::Arc;

use serde::Deserialize;
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::{ScriptedChatModel, StructuredOutputChatModel};

#[derive(Debug, Deserialize)]
struct MovieReview {
    title: String,
    rating: f32,
    summary: String,
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // The scripted model returns a JSON string that matches our schema
    let inner = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            r#"{"title": "The Matrix", "rating": 9.5, "summary": "A groundbreaking sci-fi film about simulated reality"}"#,
        ),
        usage: None,
    }]);

    let structured: StructuredOutputChatModel<MovieReview> = StructuredOutputChatModel::new(
        Arc::new(inner),
        "Extract a movie review with title (string), rating (float 0-10), and summary (string)",
    );

    // --- Use as ChatModel ---
    println!("=== Structured Output ===");
    let request = ChatRequest::new(vec![Message::human("Review The Matrix")]);
    let response = structured.chat(request).await?;
    println!("Raw response: {}", response.message.content());

    // --- Parse the response ---
    let review: MovieReview = structured.parse_response(&response)?;
    println!("\nParsed review:");
    println!("  Title:   {}", review.title);
    println!("  Rating:  {}/10", review.rating);
    println!("  Summary: {}", review.summary);

    // --- Use generate() for combined call + parse ---
    println!("\n=== Using generate() ===");
    let inner2 = ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai(
            r#"{"title": "Inception", "rating": 9.0, "summary": "A mind-bending thriller about dreams within dreams"}"#,
        ),
        usage: None,
    }]);
    let structured2: StructuredOutputChatModel<MovieReview> =
        StructuredOutputChatModel::new(Arc::new(inner2), "Extract a movie review");

    let request2 = ChatRequest::new(vec![Message::human("Review Inception")]);
    let (review2, _response) = structured2.generate(request2).await?;
    println!("Title:   {}", review2.title);
    println!("Rating:  {}/10", review2.rating);
    println!("Summary: {}", review2.summary);

    println!("\nStructured output demo completed successfully!");
    Ok(())
}
