# Langfuse

[Langfuse](https://langfuse.com/) is an open-source LLM observability and analytics platform.
This integration records Synaptic run events (LLM calls, tool invocations, chain steps)
as Langfuse traces for debugging, cost monitoring, and quality evaluation.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["langfuse"] }
```

Sign up at [cloud.langfuse.com](https://cloud.langfuse.com/) or self-host.

## Configuration

```rust,ignore
use synaptic::langfuse::{LangfuseCallback, LangfuseConfig};

let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...");
let callback = LangfuseCallback::new(config).await.unwrap();
```

### Self-Hosted Instance

```rust,ignore
let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...")
    .with_host("https://langfuse.your-company.com")
    .with_flush_batch_size(50);
```

## Usage

```rust,ignore
use synaptic::langfuse::{LangfuseCallback, LangfuseConfig};
use std::sync::Arc;

let config = LangfuseConfig::new("pk-lf-...", "sk-lf-...");
let callback = Arc::new(LangfuseCallback::new(config).await.unwrap());
// Events are buffered and auto-flushed when batch_size is reached.
// At application shutdown, flush remaining events:
callback.flush().await.unwrap();
```

## Configuration Reference

| Field | Default | Description |
|-------|---------|-------------|
| `public_key` | required | Langfuse public key |
| `secret_key` | required | Langfuse secret key |
| `host` | https://cloud.langfuse.com | Langfuse host URL |
| `flush_batch_size` | 20 | Events buffered before auto-flush |
