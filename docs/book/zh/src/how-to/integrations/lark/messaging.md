# 消息 & Bot

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
    "action": "send",
    "receive_id_type": "chat_id",
    "receive_id": "oc_xxx",
    "msg_type": "text",
    "content": "来自 Synaptic Agent 的问候！"
})).await?;

println!("消息 ID: {}", result["message_id"]);
```

### 操作说明

| 操作 | 必填字段 | 说明 |
|------|---------|------|
| `send`（默认） | `receive_id_type`, `receive_id`, `msg_type`, `content` | 发送新消息 |
| `update` | `message_id`, `msg_type`, `content` | 更新已有消息 |
| `delete` | `message_id` | 撤回消息 |

### 参数说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `receive_id_type` | 字符串 | `"chat_id"` \| `"user_id"` \| `"email"` \| `"open_id"` |
| `receive_id` | 字符串 | 与 receive_id_type 对应的接收方 ID |
| `msg_type` | 字符串 | `"text"` \| `"post"`（富文本）\| `"interactive"`（卡片） |
| `content` | 字符串 | text 类型为纯文本，post/interactive 类型为 JSON 字符串 |

---

## LarkEventListener

订阅飞书 Webhook 事件，内置 HMAC-SHA256 签名验证和 URL challenge 自动响应。通过事件名称注册类型化的处理函数。

```rust,ignore
use synaptic::lark::{LarkConfig, LarkEventListener};

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let listener = LarkEventListener::new(config)
    .on("im.message.receive_v1", |event| async move {
        let msg = &event["event"]["message"]["content"];
        println!("收到消息: {}", msg);
        Ok(())
    });

// 绑定到 0.0.0.0:8080 并开始提供 Webhook 回调服务
listener.serve("0.0.0.0:8080").await?;
```

---

## Bot 框架

Bot 功能需要开启 `bot` feature。

```toml
[dependencies]
synaptic-lark = { version = "0.2", features = ["bot"] }
```

### LarkBotClient

通过飞书 Bot API 发送消息、回复消息并查询机器人信息。

```rust,ignore
use synaptic::lark::{LarkBotClient, LarkConfig};

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let bot = LarkBotClient::new(config);

// 向群聊发送文本消息
bot.send_text("oc_xxx", "来自 Synaptic 的问候！").await?;

// 回复某条消息所在的会话
bot.reply_text("om_xxx", "收到，正在处理...").await?;

// 获取机器人自身信息
let info = bot.get_bot_info().await?;
println!("机器人名称: {}", info["bot"]["app_name"]);
```

### LarkLongConnListener

通过 WebSocket 长连接接入飞书，无需公网 IP 或 Webhook 域名。内置 LRU 去重缓存，防止重复消费同一事件。

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

## 流式卡片输出

对于 AI Agent 场景，一次性文本回复往往太慢——用户期望看到实时流式输出（打字机效果）。飞书通过 **CardKit 卡片实体** 支持此功能：创建卡片，将其作为消息发送，然后渐进式更新卡片内容，没有编辑次数限制。

> **为什么用卡片而不是消息编辑？** 飞书对单条消息的编辑次数有隐性上限（约 20-30 次），而通过 CardKit 的卡片实体更新则没有此限制。

### StreamingCardWriter

`StreamingCardWriter` 管理完整的流式生命周期：创建卡片 → 发送/回复 → 节流更新 → 完成。

```rust,ignore
use synaptic::lark::{LarkConfig, LarkBotClient};
use synaptic::lark::bot::StreamingCardOptions;

let config = LarkConfig::new("cli_xxx", "secret_xxx");
let client = LarkBotClient::new(config);

// 开始流式卡片回复
let opts = StreamingCardOptions::new().with_title("AI 回复");
let writer = client.streaming_reply("om_原始消息ID", opts).await?;

// 增量写入内容（更新间隔节流至约 500ms）
writer.write("思考中").await?;
writer.write("...").await?;
writer.write("\n\n答案是：**42**").await?;

// 完成 — 发送最后一次缓冲更新
writer.finish().await?;
```

### 配置选项

| 方法 | 默认值 | 说明 |
|------|--------|------|
| `with_title(s)` | `""` | 卡片标题（空字符串则不显示标题栏） |
| `with_throttle(dur)` | 500ms | 卡片更新最小间隔 |

### 发送 vs 回复

```rust,ignore
// 发送到群聊（新消息）
let writer = client.streaming_send("chat_id", "oc_xxx", opts).await?;

// 回复某条消息
let writer = client.streaming_reply("om_xxx", opts).await?;
```

### 低级卡片 API

对于高级用法，可以直接使用卡片 API：

```rust,ignore
use synaptic::lark::bot::{build_card_json, build_card_json_streaming};

// ── 静态卡片（无打字机效果）────────────────────────────────────
let card = build_card_json("标题", "初始内容");
let card_id = client.create_card(&card).await?;

// 全量卡片更新（递增序列号）
let updated = build_card_json("标题", "更新后的内容");
client.update_card(&card_id, 1, &updated).await?;

// ── 流式卡片（打字机动画）──────────────────────────────────────
let streaming_card = build_card_json_streaming("标题", "", true);
let card_id = client.create_card(&streaming_card).await?;

// 元素级内容流式更新——产生打字机效果
// content 必须是完整的累积文本（不是增量 delta）。
// 如果新文本是旧文本的前缀扩展，只有新增字符会产生动画效果。
client.stream_card_content(&card_id, "streaming_content", "你好", 1).await?;
client.stream_card_content(&card_id, "streaming_content", "你好世界", 2).await?;

// 最终：全量卡片更新 + streaming_mode: false 停止 "生成中..." 指示器
client.update_card(&card_id, 3, &build_card_json_streaming("标题", "你好世界！", false)).await?;
```

`StreamingCardWriter` 自动管理整个生命周期——以 `streaming_mode: true` 创建卡片，通过元素 API 流式输出内容，最终以 `streaming_mode: false` 结束。

### Card JSON 2.0 结构

卡片使用飞书 Card JSON 2.0 格式：

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
    "title": { "tag": "plain_text", "content": "AI 回复" }
  },
  "body": {
    "elements": [
      {
        "tag": "markdown",
        "content": "流式文本内容...",
        "element_id": "streaming_content"
      }
    ]
  }
}
```

关键字段：
- `update_multi: true` — 开启卡片无限次更新
- `streaming_mode: true` — 启用客户端打字机动画；最终更新时设为 `false`
- `streaming_config` — 控制动画速度：`print_frequency_ms`（打印间隔毫秒数）、`print_step`（每步字符数）、`print_strategy`（`"fast"` 或 `"delay"`）
- `element_id` — 每个组件的唯一标识符，流式更新时必填
- `body.elements[0].content` — 每次写入时更新的 Markdown 内容
- `sequence` — 每张卡片严格递增的序列号（由 `StreamingCardWriter` 自动管理）

### 流式 Bot 示例

完整示例请参考 `examples/lark_streaming_bot/`。
