# ModelFallbackMiddleware

Falls back to alternative models when the primary model fails. Use this for high-availability scenarios where you want seamless failover between providers (e.g., OpenAI to Anthropic) or between model tiers (e.g., GPT-4 to GPT-3.5).

## Constructor

```rust,ignore
use synaptic::middleware::ModelFallbackMiddleware;

let mw = ModelFallbackMiddleware::new(vec![
    fallback_model_1,  // Arc<dyn ChatModel>
    fallback_model_2,  // Arc<dyn ChatModel>
]);
```

The fallback list is tried in order. The first successful response is returned.

## Usage with `create_agent`

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{create_agent, AgentOptions};
use synaptic::openai::OpenAiChatModel;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::middleware::ModelFallbackMiddleware;

let primary = Arc::new(OpenAiChatModel::new("gpt-4o"));
let fallback = Arc::new(AnthropicChatModel::new("claude-sonnet-4-20250514"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelFallbackMiddleware::new(vec![fallback])),
    ],
    ..Default::default()
};

let graph = create_agent(primary, tools, options)?;
```

## How It Works

- **Lifecycle hook:** `wrap_model_call`
- The middleware first delegates to `next.call(request)`, which calls the primary model through the rest of the middleware chain.
- If the primary call succeeds, the response is returned as-is.
- If the primary call fails, the middleware tries each fallback model in order by creating a `BaseChatModelCaller` and sending the same request.
- The first fallback that succeeds is returned. If all fallbacks also fail, the original error from the primary model is returned.

Fallback models are called directly (bypassing the middleware chain) to avoid interference from other middlewares that may have caused or contributed to the failure.

## Example: Multi-tier Fallback

```rust,ignore
let primary = Arc::new(OpenAiChatModel::new("gpt-4o"));
let tier2 = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));
let tier3 = Arc::new(AnthropicChatModel::new("claude-sonnet-4-20250514"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelFallbackMiddleware::new(vec![tier2, tier3])),
    ],
    ..Default::default()
};

let graph = create_agent(primary, tools, options)?;
```

The agent tries GPT-4o first, then GPT-4o-mini, then Claude Sonnet.
