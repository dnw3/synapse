# 飞书 / Lark 集成

`synaptic-lark` crate 将 Synaptic 与[飞书开放平台](https://open.feishu.cn/)深度集成，提供文档加载器和 Agent 工具，用于与飞书各类服务进行交互。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["lark"] }
```

在[飞书开发者后台](https://open.feishu.cn/app)创建自定义应用，获取 **App ID** 和 **App Secret**，并开启所需权限（详见[权限说明](#权限说明)）。

## 配置

```rust,ignore
use synaptic::lark::LarkConfig;

// 飞书公有云（默认）
let config = LarkConfig::new("cli_xxx", "app_secret_xxx");

// 字节跳动内网
let config = LarkConfig::new("cli_xxx", "app_secret_xxx")
    .with_base_url("https://fsopen.bytedance.net/open-apis");
```

`tenant_access_token` 会自动获取和刷新——token 有效期为 7,200 秒，剩余不足 300 秒时自动续期。

---

## LarkDocLoader

将飞书文档和知识库页面加载为 Synaptic [`Document`]，可直接用于 RAG 管道。

```rust,ignore
use synaptic::lark::{LarkConfig, LarkDocLoader};
use synaptic::core::Loader;

let config = LarkConfig::new("cli_xxx", "secret_xxx");

// 加载指定文档 token
let loader = LarkDocLoader::new(config.clone())
    .with_doc_tokens(vec!["doxcnAbcXxx".to_string()]);

// 或遍历整个 Wiki 空间
let loader = LarkDocLoader::new(config)
    .with_wiki_space_id("spcXxx");

let docs = loader.load().await?;
for doc in &docs {
    println!("标题: {}", doc.metadata["title"]);
    println!("URL:  {}", doc.metadata["url"]);
    println!("长度: {} 字符", doc.content.len());
}
```

### 文档 Metadata 字段

| 字段 | 说明 |
|------|------|
| `doc_id` | 飞书文档 token |
| `title` | 文档标题 |
| `source` | `lark:doc:<token>` |
| `url` | 飞书文档直链 |
| `doc_type` | 固定为 `"docx"` |

### 构建器选项

| 方法 | 说明 |
|------|------|
| `with_doc_tokens(tokens)` | 加载指定文档 token 列表 |
| `with_wiki_space_id(id)` | 遍历 Wiki 空间内的所有文档 |

---

## LarkMessageTool

作为 Agent 工具，向飞书群聊或用户发送消息。

```rust,ignore
use synaptic::lark::{LarkConfig, LarkMessageTool};
use synaptic::core::Tool;
use serde_json::json;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let tool = LarkMessageTool::new(config);

// 发送文本消息
let result = tool.call(json!({
    "receive_id_type": "chat_id",
    "receive_id": "oc_xxx",
    "msg_type": "text",
    "content": "来自 Synaptic Agent 的问候！"
})).await?;

println!("消息 ID: {}", result["message_id"]);
```

### 参数说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `receive_id_type` | 字符串 | ✅ | `"chat_id"` \| `"user_id"` \| `"email"` \| `"open_id"` |
| `receive_id` | 字符串 | ✅ | 与 receive_id_type 对应的接收方 ID |
| `msg_type` | 字符串 | ✅ | `"text"` \| `"post"`（富文本）\| `"interactive"`（卡片） |
| `content` | 字符串 | ✅ | text 类型为纯文本，post/interactive 类型为 JSON 字符串 |

---

## LarkBitableTool

对飞书多维表格（Bitable）执行查询、新增和更新操作。

```rust,ignore
use synaptic::lark::{LarkBitableTool, LarkConfig};
use synaptic::core::Tool;
use serde_json::json;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let tool = LarkBitableTool::new(config);

// 查询记录
let records = tool.call(json!({
    "action": "search",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "filter": { "field": "状态", "value": "待处理" }
})).await?;

// 新建记录
let created = tool.call(json!({
    "action": "create",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "records": [{ "任务": "新事项", "状态": "进行中" }]
})).await?;

// 更新记录
let updated = tool.call(json!({
    "action": "update",
    "app_token": "bascnXxx",
    "table_id": "tblXxx",
    "record_id": "recXxx",
    "fields": { "状态": "完成" }
})).await?;
```

### 操作说明

| 操作 | 额外必填字段 | 说明 |
|------|------------|------|
| `search` | `filter?`（可选）| 查询记录，支持字段过滤 |
| `create` | `records` | 批量新建记录 |
| `update` | `record_id`, `fields` | 更新指定记录的字段 |

---

## 与 ReAct Agent 结合

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
    synaptic::graph::MessageState::from("查询所有待处理任务并将摘要发送到群聊 oc_xxx"),
).await?;
```

---

## 权限说明

在飞书开发者后台的**权限与范围**页面开启以下权限：

| 功能 | 所需权限 |
|------|---------|
| LarkDocLoader（文档） | `docx:document:readonly` |
| LarkDocLoader（知识库） | `wiki:wiki:readonly` |
| LarkMessageTool | `im:message:send_as_bot` |
| LarkBitableTool（只读） | `bitable:app:readonly` |
| LarkBitableTool（读写） | `bitable:app` |
