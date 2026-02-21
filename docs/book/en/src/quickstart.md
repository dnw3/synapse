# Quickstart

This guide walks you through a minimal Synaptic program that sends a chat request and prints the response. It uses `ScriptedChatModel`, a test double that returns pre-configured responses, so you do not need any API keys to run it.

## The Complete Example

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::ScriptedChatModel;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // 1. Create a scripted model with a predefined response.
    //    ScriptedChatModel returns responses in order, one per chat() call.
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("Hello! I'm a Synaptic assistant. How can I help you today?"),
            usage: None,
        },
    ]);

    // 2. Build a chat request with a system prompt and a user message.
    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant built with Synaptic."),
        Message::human("Hello! What are you?"),
    ]);

    // 3. Send the request and get a response.
    let response = model.chat(request).await?;

    // 4. Print the assistant's reply.
    println!("Assistant: {}", response.message.content());

    Ok(())
}
```

Running this program prints:

```text
Assistant: Hello! I'm a Synaptic assistant. How can I help you today?
```

## What is Happening

1. **`ScriptedChatModel::new(vec![...])`** creates a chat model that returns the given `ChatResponse` values in sequence. This is useful for testing and examples without requiring a live API. In production, you would replace this with `OpenAiChatModel` (from `synaptic::openai`), `AnthropicChatModel` (from `synaptic::anthropic`), or another provider adapter.

2. **`ChatRequest::new(messages)`** constructs a chat request from a vector of messages. Messages are created with factory methods: `Message::system()` for system prompts, `Message::human()` for user input, and `Message::ai()` for assistant responses.

3. **`model.chat(request).await?`** sends the request asynchronously and returns a `ChatResponse` containing the model's message and optional token usage information.

4. **`response.message.content()`** extracts the text content from the response message.

## Using a Real Provider

To use OpenAI instead of the scripted model, replace the model creation:

```rust
use synaptic::openai::OpenAiChatModel;

// Reads OPENAI_API_KEY from the environment automatically.
let model = OpenAiChatModel::new("gpt-4o");
```

You will also need the `"openai"` feature enabled in your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai"] }
```

The rest of the code stays the same -- `ChatModel::chat()` has the same signature regardless of provider.

## Next Steps

- [Build a Simple LLM Application](tutorials/simple-llm-app.md) -- Chain prompts with output parsers
- [Build a Chatbot with Memory](tutorials/chatbot-with-memory.md) -- Add conversation history
- [Build a ReAct Agent](tutorials/react-agent.md) -- Give your model tools to call
- [Build a RAG Application](tutorials/rag-application.md) -- Retrieve documents for context
- [Architecture Overview](architecture-overview.md) -- Understand the crate structure
