# SummarizationMiddleware

Automatically summarizes conversation history when it exceeds a token limit. Use this for long-running agents where the context window would otherwise overflow, replacing older messages with a concise summary while keeping recent messages intact.

## Constructor

```rust,ignore
use synaptic::middleware::SummarizationMiddleware;

let mw = SummarizationMiddleware::new(
    summarizer_model,   // Arc<dyn ChatModel> -- model used to generate summaries
    4000,               // max_tokens -- threshold that triggers summarization
    |msg: &Message| {   // token_counter -- estimates tokens per message
        msg.content().len() / 4
    },
);
```

**Parameters:**

- `model` -- The ChatModel used to generate the summary. Can be the same model as the agent or a cheaper/faster one.
- `max_tokens` -- When the estimated total tokens exceed this value, summarization is triggered.
- `token_counter` -- A function `Fn(&Message) -> usize` that estimates the token count for a single message. A common heuristic is `content.len() / 4`.

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{create_agent, AgentOptions};
use synaptic::middleware::SummarizationMiddleware;
use synaptic::openai::OpenAiChatModel;

let summarizer = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            summarizer,
            4000,
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `before_model`
- Before each model call, the middleware sums the estimated tokens across all messages.
- If the total is within `max_tokens`, no action is taken.
- If the total exceeds the limit, it splits messages into two groups:
  - **Recent messages** that fit within half the token budget (kept as-is).
  - **Older messages** that are sent to the summarizer model.
- The summarizer produces a concise summary, which replaces the older messages as a system message prefixed with `[Previous conversation summary]`.
- The request then proceeds with the summary plus the recent messages, staying within budget.

This approach preserves the most recent context verbatim while compressing older exchanges, keeping the agent informed about prior conversation without exceeding context limits.

## Example: Budget-conscious Summarization

Use a cheaper model for summaries to reduce costs:

```rust,ignore
use synaptic::openai::OpenAiChatModel;

let agent_model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let cheap_model = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            cheap_model,
            8000,
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(agent_model, tools, options)?;
```

## Offline Testing with ScriptedChatModel

Test summarization behavior without API keys:

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatResponse, Message};
use synaptic::models::ScriptedChatModel;
use synaptic::middleware::SummarizationMiddleware;
use synaptic::graph::{create_agent, AgentOptions, MessageState};

// Script: summarizer returns a summary, agent responds
let summarizer = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Summary: discussed Rust ownership."),
        usage: None,
    },
]));

let agent_model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Here's more about lifetimes."),
        usage: None,
    },
]));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            summarizer,
            100,  // low threshold for testing
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(agent_model, vec![], options)?;

// Build a state with enough messages to exceed the threshold
let mut state = MessageState::new();
state.messages.push(Message::human("What is Rust?"));
state.messages.push(Message::ai("Rust is a systems programming language..."));
state.messages.push(Message::human("Tell me about ownership."));
state.messages.push(Message::ai("Ownership is a set of rules that govern memory..."));
state.messages.push(Message::human("And lifetimes?"));

let result = graph.invoke(state).await?.into_state();
```
