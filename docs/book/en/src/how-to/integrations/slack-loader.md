# Slack Loader

Load messages from Slack channels into Synaptic documents using the Slack Web API.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["slack"] }
```

Create a Slack app at [api.slack.com/apps](https://api.slack.com/apps), add the `channels:history` OAuth scope, install it to your workspace, and copy the Bot Token (`xoxb-...`).

## Usage

```rust,ignore
use synaptic::slack::{SlackConfig, SlackLoader};
use synaptic::core::Loader;

let config = SlackConfig::new(
    "xoxb-your-bot-token",
    vec!["C1234567890".to_string(), "C0987654321".to_string()],
)
.with_limit(200)
.with_oldest("1700000000.000000"); // Unix timestamp as string
let loader = SlackLoader::new(config);

let docs = loader.load().await?;
for doc in &docs {
    println!("[{}] {}: {}", doc.metadata["channel"], doc.metadata["user"], doc.content);
}
```

## Configuration

| Method | Description |
|--------|-------------|
| `with_limit(n)` | Maximum messages to fetch per channel (default: 100) |
| `with_oldest(ts)` | Only fetch messages after this Unix timestamp string |
| `with_threads()` | Include thread replies (metadata `thread_ts` field populated) |

## Metadata Fields

Each document includes:

- `source` — `slack:<channel-id>`
- `channel` — the channel ID
- `ts` — message timestamp (Slack format, also used as sort key)
- `user` — Slack user ID of the sender
- `thread_ts` — parent thread timestamp (if message is part of a thread)
