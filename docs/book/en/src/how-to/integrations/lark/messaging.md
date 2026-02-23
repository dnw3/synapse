# Messaging & Bot

## LarkMessageTool

Send messages to Feishu chats or users as an Agent tool.

```rust,ignore
use synaptic::lark::{LarkConfig, LarkMessageTool};
use synaptic::core::Tool;
use serde_json::json;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let tool = LarkMessageTool::new(config);

// Text message
let result = tool.call(json!({
    "action": "send",
    "receive_id_type": "chat_id",
    "receive_id": "oc_xxx",
    "msg_type": "text",
    "content": "Hello from Synaptic Agent!"
})).await?;

println!("Sent message ID: {}", result["message_id"]);
```

### Actions

| Action | Required fields | Description |
|--------|----------------|-------------|
| `send` (default) | `receive_id_type`, `receive_id`, `msg_type`, `content` | Send a new message |
| `update` | `message_id`, `msg_type`, `content` | Update an existing message |
| `delete` | `message_id` | Delete (recall) a message |

### Parameters

| Field | Type | Description |
|-------|------|-------------|
| `receive_id_type` | string | `"chat_id"` \| `"user_id"` \| `"email"` \| `"open_id"` |
| `receive_id` | string | The receiver ID matching the type |
| `msg_type` | string | `"text"` \| `"post"` (rich text) \| `"interactive"` (card) |
| `content` | string | Plain string for text; JSON string for post/interactive |

---

## LarkEventListener

Subscribe to Feishu webhook events with HMAC-SHA256 signature verification and automatic URL challenge handling. Register typed event handlers by event name.

```rust,ignore
use synaptic::lark::{LarkConfig, LarkEventListener};

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let listener = LarkEventListener::new(config)
    .on("im.message.receive_v1", |event| async move {
        let msg = &event["event"]["message"]["content"];
        println!("Received: {}", msg);
        Ok(())
    });

// Bind to 0.0.0.0:8080 and start serving webhook callbacks
listener.serve("0.0.0.0:8080").await?;
```

---

## Bot Framework

The bot features require the `bot` feature flag.

```toml
[dependencies]
synaptic-lark = { version = "0.2", features = ["bot"] }
```

### LarkBotClient

Send and reply to messages, and query bot information via the Feishu Bot API.

```rust,ignore
use synaptic::lark::{LarkBotClient, LarkConfig};

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let bot = LarkBotClient::new(config);

// Send a text message to a chat
bot.send_text("oc_xxx", "Hello from Synaptic!").await?;

// Reply to an existing message thread
bot.reply_text("om_xxx", "Got it, processing now...").await?;

// Get information about the bot itself
let info = bot.get_bot_info().await?;
println!("Bot name: {}", info["bot"]["app_name"]);
```

### LarkLongConnListener

Connect to Feishu using a WebSocket long-connection so that no public IP or webhook endpoint is required. Incoming events are deduplicated via an internal LRU cache.

```rust,ignore
use synaptic::lark::{LarkConfig, LarkLongConnListener, MessageHandler};
use synaptic::core::Message;
use async_trait::async_trait;

struct EchoHandler;

#[async_trait]
impl MessageHandler for EchoHandler {
    async fn handle(&self, event: serde_json::Value) -> anyhow::Result<()> {
        let text = event["event"]["message"]["content"].as_str().unwrap_or("");
        println!("Echo: {text}");
        Ok(())
    }
}

let config = LarkConfig::new("cli_xxx", "secret_xxx");
LarkLongConnListener::new(config)
    .with_message_handler(EchoHandler)
    .run()
    .await?;
```

---

## Streaming Card Output

For AI agents, one-shot text replies are often too slow — users expect to see responses streaming in real-time (typewriter effect). Feishu supports this via **CardKit card entities**: create a card, send it as a message, then progressively update the card content with no edit-count limit.

> **Why cards instead of message edits?** Feishu imposes a hidden limit (~20-30) on message edits per message. Card entities via CardKit have no such limit.

### StreamingCardWriter

The `StreamingCardWriter` manages the full streaming lifecycle: create card → send/reply → throttled updates → finalize.

```rust,ignore
use synaptic::lark::{LarkConfig, LarkBotClient};
use synaptic::lark::bot::StreamingCardOptions;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let client = LarkBotClient::new(config);

// Start a streaming card reply
let opts = StreamingCardOptions::new().with_title("AI Response");
let writer = client.streaming_reply("om_original_message_id", opts).await?;

// Write content incrementally (throttled to ~500ms between updates)
writer.write("Thinking").await?;
writer.write("...").await?;
writer.write("\n\nHere is the answer: **42**").await?;

// Finalize — sends the last buffered update
writer.finish().await?;
```

### Options

| Method | Default | Description |
|--------|---------|-------------|
| `with_title(s)` | `""` | Card header title (empty = no header) |
| `with_throttle(dur)` | 500ms | Minimum interval between card updates |

### Send vs Reply

```rust,ignore
// Send to a chat (new message)
let writer = client.streaming_send("chat_id", "oc_xxx", opts).await?;

// Reply in a thread
let writer = client.streaming_reply("om_xxx", opts).await?;
```

### Low-Level Card API

For advanced use cases, you can use the card APIs directly:

```rust,ignore
use synaptic::lark::bot::{build_card_json, build_card_json_streaming};

// ── Static card (no typewriter) ──────────────────────────────────
let card = build_card_json("Title", "Initial content");
let card_id = client.create_card(&card).await?;

// Full card update with incrementing sequence
let updated = build_card_json("Title", "Updated content");
client.update_card(&card_id, 1, &updated).await?;

// ── Streaming card (typewriter animation) ────────────────────────
let streaming_card = build_card_json_streaming("Title", "", true);
let card_id = client.create_card(&streaming_card).await?;

// Element-level content streaming — produces typewriter effect
// Content must be the full accumulated text (not a delta).
// If the new text extends the old text, only the new characters animate.
client.stream_card_content(&card_id, "streaming_content", "Hello", 1).await?;
client.stream_card_content(&card_id, "streaming_content", "Hello World", 2).await?;

// Final: full card update with streaming_mode: false to stop "Generating..." indicator
client.update_card(&card_id, 3, &build_card_json_streaming("Title", "Hello World!", false)).await?;
```

`StreamingCardWriter` handles this lifecycle automatically — create with `streaming_mode: true`, stream content via the element API, and finalize with `streaming_mode: false`.

### Card JSON 2.0 Structure

Cards use Feishu's Card JSON 2.0 schema:

```json
{
  "schema": "2.0",
  "config": {
    "update_multi": true,
    "streaming_mode": true,
    "streaming_config": {
      "print_frequency_ms": { "default": 30 },
      "print_step": { "default": 2 },
      "print_strategy": "fast"
    }
  },
  "header": {
    "title": { "tag": "plain_text", "content": "AI Response" }
  },
  "body": {
    "elements": [
      {
        "tag": "markdown",
        "content": "Streaming text here...",
        "element_id": "streaming_content"
      }
    ]
  }
}
```

Key fields:
- `update_multi: true` — enables unlimited updates to the card
- `streaming_mode: true` — enables client-side typewriter animation; set to `false` on final update
- `streaming_config` — controls animation speed: `print_frequency_ms` (ms between prints), `print_step` (characters per step), `print_strategy` (`"fast"` or `"delay"`)
- `element_id` — unique identifier for each component, required for streaming updates
- `body.elements[0].content` — Markdown content updated on each write
- `sequence` — strictly incrementing counter per card (managed by `StreamingCardWriter`)

### Streaming Bot Example

See the complete example at `examples/lark_streaming_bot/`.
