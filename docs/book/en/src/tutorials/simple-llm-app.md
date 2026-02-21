# Build a Simple LLM Application

This tutorial walks you through building a basic chat application with Synaptic. You will learn how to create a chat model, send messages, template prompts, and compose processing pipelines using the LCEL pipe operator.

## Prerequisites

Add the required Synaptic crates to your `Cargo.toml`:

```toml
[dependencies]
synaptic = "0.2"
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Step 1: Create a Chat Model

Every LLM interaction in Synaptic goes through a type that implements the `ChatModel` trait. For production use you would reach for `OpenAiChatModel` (from `synaptic::openai`), `AnthropicChatModel` (from `synaptic::anthropic`), or one of the other provider adapters. For this tutorial we use `ScriptedChatModel`, which returns pre-configured responses -- perfect for offline development and testing.

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message};
use synaptic::models::ScriptedChatModel;

let model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Paris is the capital of France."),
        usage: None,
    },
]);
```

`ScriptedChatModel` pops responses from a queue in order. Each call to `chat()` returns the next response. This makes tests deterministic and lets you compile and run examples without an API key.

## Step 2: Build a Request and Get a Response

A `ChatRequest` holds the conversation messages (and optionally tool definitions). Build one with `ChatRequest::new()` and pass a vector of messages:

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message};
use synaptic::models::ScriptedChatModel;

#[tokio::main]
async fn main() {
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("Paris is the capital of France."),
            usage: None,
        },
    ]);

    let request = ChatRequest::new(vec![
        Message::system("You are a geography expert."),
        Message::human("What is the capital of France?"),
    ]);

    let response = model.chat(request).await.unwrap();
    println!("{}", response.message.content());
    // Output: Paris is the capital of France.
}
```

Key points:

- `Message::system()`, `Message::human()`, and `Message::ai()` are factory methods for building typed messages.
- `ChatRequest::new(messages)` is the constructor. Never build the struct literal directly.
- `model.chat(request)` is async and returns `Result<ChatResponse, SynapticError>`.

## Step 3: Template Messages with ChatPromptTemplate

Hard-coding message strings works for one-off calls, but real applications need parameterized prompts. `ChatPromptTemplate` lets you define message templates with `{{ variable }}` placeholders that are filled in at runtime.

```rust
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate};

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a helpful assistant that speaks {{ language }}."),
    MessageTemplate::human("{{ question }}"),
]);
```

To render the template, call `format()` with a map of variable values:

```rust
use std::collections::HashMap;
use serde_json::Value;
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate};

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a helpful assistant that speaks {{ language }}."),
    MessageTemplate::human("{{ question }}"),
]);

let mut values = HashMap::new();
values.insert("language".to_string(), Value::String("French".to_string()));
values.insert("question".to_string(), Value::String("What is the capital of France?".to_string()));

let messages = template.format(&values).unwrap();
// messages[0] => System("You are a helpful assistant that speaks French.")
// messages[1] => Human("What is the capital of France?")
```

`ChatPromptTemplate` also implements the `Runnable` trait, which means it can participate in LCEL pipelines. When used as a `Runnable`, it takes a `HashMap<String, Value>` as input and produces `Vec<Message>` as output.

## Step 4: Compose a Pipeline with the Pipe Operator

Synaptic implements LangChain Expression Language (LCEL) composition through the `|` pipe operator. You can chain any two runnables together as long as the output type of the first matches the input type of the second.

Here is a complete example that templates a prompt and extracts the response text:

```rust
use std::collections::HashMap;
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, RunnableConfig};
use synaptic::models::ScriptedChatModel;
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic::parsers::StrOutputParser;
use synaptic::runnables::Runnable;

#[tokio::main]
async fn main() {
    // 1. Define the model
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("The capital of France is Paris."),
            usage: None,
        },
    ]);

    // 2. Define the prompt template
    let template = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::system("You are a geography expert."),
        MessageTemplate::human("{{ question }}"),
    ]);

    // 3. Build the chain: template -> model -> parser
    //    Each step is boxed to erase types, then piped with |
    let chain = template.boxed() | model.boxed() | StrOutputParser.boxed();

    // 4. Invoke the chain
    let mut input = HashMap::new();
    input.insert(
        "question".to_string(),
        serde_json::Value::String("What is the capital of France?".to_string()),
    );

    let config = RunnableConfig::default();
    let result: String = chain.invoke(input, &config).await.unwrap();
    println!("{}", result);
    // Output: The capital of France is Paris.
}
```

Here is what happens at each stage of the pipeline:

1. **`ChatPromptTemplate`** receives `HashMap<String, Value>`, renders the templates, and outputs `Vec<Message>`.
2. **`ScriptedChatModel`** receives `Vec<Message>` (via its `Runnable` implementation which wraps them in a `ChatRequest`), calls the model, and outputs a `Message`.
3. **`StrOutputParser`** receives a `Message` and extracts its text content as a `String`.

The `boxed()` method wraps each component into a `BoxRunnable`, which is a type-erased wrapper that enables the `|` operator. Without boxing, Rust cannot unify the different concrete types.

## Summary

In this tutorial you learned how to:

- Create a `ScriptedChatModel` for offline development
- Build `ChatRequest` objects from typed messages
- Use `ChatPromptTemplate` with `{{ variable }}` interpolation
- Compose processing pipelines with the LCEL `|` pipe operator

## Next Steps

- [Build a Chatbot with Memory](chatbot-with-memory.md) -- add conversation history
- [Build a ReAct Agent](react-agent.md) -- give the LLM tools to call
- [Runnables & LCEL](../concepts/runnables-lcel.md) -- deeper look at composition patterns
