# Feishu / Lark Integration

The `synaptic-lark` crate integrates Synaptic with the [Feishu/Lark Open Platform](https://open.feishu.cn/), providing document loaders and Agent tools for interacting with Feishu services.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["lark"] }
```

Create a custom app at the [Feishu Developer Console](https://open.feishu.cn/app), obtain your **App ID** and **App Secret**, and enable the required scopes (see [Permissions](#permissions) below).

## Configuration

```rust,ignore
use synaptic::lark::LarkConfig;

// Public Feishu cloud (default)
let config = LarkConfig::new("cli_xxx", "app_secret_xxx");

// ByteDance internal network
let config = LarkConfig::new("cli_xxx", "app_secret_xxx")
    .with_base_url("https://fsopen.bytedance.net/open-apis");
```

The `tenant_access_token` is fetched and refreshed automatically — tokens are valid for 7,200 seconds and are renewed when fewer than 300 seconds remain.

---

## LarkDocLoader

Load Feishu documents and Wiki pages into Synaptic [`Document`]s for RAG pipelines.

```rust,ignore
use synaptic::lark::{LarkConfig, LarkDocLoader};
use synaptic::core::Loader;

let config = LarkConfig::new("cli_xxx", "secret_xxx");

// Load specific document tokens
let loader = LarkDocLoader::new(config.clone())
    .with_doc_tokens(vec!["doxcnAbcXxx".to_string()]);

// Or traverse an entire Wiki space
let loader = LarkDocLoader::new(config)
    .with_wiki_space_id("spcXxx");

let docs = loader.load().await?;
for doc in &docs {
    println!("Title: {}", doc.metadata["title"]);
    println!("URL:   {}", doc.metadata["url"]);
    println!("Length: {} chars", doc.content.len());
}
```

### Document Metadata

Each document includes:

| Field | Description |
|-------|-------------|
| `doc_id` | The Feishu document token |
| `title` | Document title |
| `source` | `lark:doc:<token>` |
| `url` | Direct Feishu document URL |
| `doc_type` | Always `"docx"` |

### Builder Options

| Method | Description |
|--------|-------------|
| `with_doc_tokens(tokens)` | Load specific document tokens |
| `with_wiki_space_id(id)` | Traverse all docs in a Wiki space |

---

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
    "receive_id_type": "chat_id",
    "receive_id": "oc_xxx",
    "msg_type": "text",
    "content": "Hello from Synaptic Agent!"
})).await?;

println!("Sent message ID: {}", result["message_id"]);
```

### Arguments

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `receive_id_type` | string | ✅ | `"chat_id"` \| `"user_id"` \| `"email"` \| `"open_id"` |
| `receive_id` | string | ✅ | The receiver ID matching the type |
| `msg_type` | string | ✅ | `"text"` \| `"post"` (rich text) \| `"interactive"` (card) |
| `content` | string | ✅ | Plain string for text; JSON string for post/interactive |

---

## LarkBitableTool

Search, create, and update records in a Feishu Bitable (multi-dimensional table).

```rust,ignore
use synaptic::lark::{LarkBitableTool, LarkConfig};
use synaptic::core::Tool;
use serde_json::json;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let tool = LarkBitableTool::new(config);

// Search records
let records = tool.call(json!({
    "action": "search",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "filter": { "field": "Status", "value": "Pending" }
})).await?;

// Create records
let created = tool.call(json!({
    "action": "create",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "records": [{ "Task": "New item", "Status": "Open" }]
})).await?;

// Update a record
let updated = tool.call(json!({
    "action": "update",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "record_id": "recXxx",
    "fields": { "Status": "Done" }
})).await?;
```

### Actions

| Action | Required extra fields | Description |
|--------|----------------------|-------------|
| `search` | `filter?` | Query records (optional field+value filter) |
| `create` | `records` | Create one or more records |
| `update` | `record_id`, `fields` | Update fields on an existing record |

---

## Using with a ReAct Agent

```rust,ignore
use synaptic::lark::{LarkBitableTool, LarkConfig, LarkMessageTool};
use synaptic::graph::create_react_agent;
use synaptic::openai::OpenAiChatModel;

let model = OpenAiChatModel::from_env();
let config = LarkConfig::new("cli_xxx", "secret_xxx");

let tools: Vec<Box<dyn synaptic::core::Tool>> = vec![
    Box::new(LarkBitableTool::new(config.clone())),
    Box::new(LarkMessageTool::new(config)),
];
let agent = create_react_agent(model, tools);

let result = agent.invoke(
    synaptic::graph::MessageState::from("Check all pending tasks and send a summary to chat oc_xxx"),
).await?;
```

---

## Permissions

Enable the following scopes in the Feishu Developer Console under **Permissions & Scopes**:

| Feature | Required Scope |
|---------|---------------|
| LarkDocLoader (documents) | `docx:document:readonly` |
| LarkDocLoader (Wiki) | `wiki:wiki:readonly` |
| LarkMessageTool | `im:message:send_as_bot` |
| LarkBitableTool (read) | `bitable:app:readonly` |
| LarkBitableTool (write) | `bitable:app` |
