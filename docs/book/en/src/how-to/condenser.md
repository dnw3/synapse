# Context Condensation

The `Condenser` trait compresses conversation history before it reaches the model, keeping context windows manageable in long-running agents.

## Condenser Trait

```rust,ignore
use synaptic::condenser::Condenser;

#[async_trait]
pub trait Condenser: Send + Sync {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError>;
}
```

## Built-in Condensers

### NoOpCondenser

Returns messages unchanged. Useful as a default or placeholder.

```rust,ignore
use synaptic::condenser::NoOpCondenser;

let condenser = NoOpCondenser;
let output = condenser.condense(messages).await?;
// output == messages (unchanged)
```

### RollingCondenser

Keeps the most recent N messages. The system message is preserved by default.

```rust,ignore
use synaptic::condenser::RollingCondenser;

let condenser = RollingCondenser::new(20)
    .with_preserve_system(true);  // default: true
```

### LlmSummarizingCondenser

Summarizes older messages using an LLM while keeping recent messages intact.

```rust,ignore
use synaptic::condenser::LlmSummarizingCondenser;

let condenser = LlmSummarizingCondenser::new(
    model.clone(),   // Arc<dyn ChatModel>
    4096,            // max_tokens threshold
    5,               // keep_recent: number of recent messages to preserve
);
```

When the estimated token count exceeds `max_tokens`, older messages are summarized into a single system message and prepended to the recent messages.

### TokenBudgetCondenser

Trims messages to fit within a token budget using a `TokenCounter`.

```rust,ignore
use synaptic::condenser::TokenBudgetCondenser;
use synaptic::core::HeuristicTokenCounter;

let counter = Arc::new(HeuristicTokenCounter);
let condenser = TokenBudgetCondenser::new(4096, counter)
    .with_include_system(true);  // preserve system message (default)
```

### PipelineCondenser

Chains multiple condensers in sequence. Each condenser's output feeds into the next.

```rust,ignore
use synaptic::condenser::{PipelineCondenser, RollingCondenser, TokenBudgetCondenser};

let pipeline = PipelineCondenser::new(vec![
    Arc::new(RollingCondenser::new(50)),
    Arc::new(TokenBudgetCondenser::new(4096, counter)),
]);
```

## CondenserMiddleware

Wraps any condenser as an `AgentMiddleware`, automatically condensing messages before each model call.

```rust,ignore
use synaptic::condenser::CondenserMiddleware;
use synaptic::condenser::RollingCondenser;

let middleware = CondenserMiddleware::new(
    Arc::new(RollingCondenser::new(20)),
);

// Use with agent options
let options = AgentOptions {
    middleware: vec![Arc::new(middleware)],
    ..Default::default()
};
```
