# Slack 加载器

使用 Slack Web API 将 Slack 频道消息加载为 Synaptic 文档。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["slack"] }
```

在 [api.slack.com/apps](https://api.slack.com/apps) 创建 Slack App，添加 `channels:history` OAuth 权限范围，安装到工作区并复制 Bot Token（`xoxb-...`）。

## 使用示例

```rust,ignore
use synaptic::slack::{SlackConfig, SlackLoader};
use synaptic::core::Loader;

let config = SlackConfig::new(
    "xoxb-your-bot-token",
    vec!["C1234567890".to_string(), "C0987654321".to_string()],
)
.with_limit(200)
.with_oldest("1700000000.000000"); // Unix 时间戳字符串
let loader = SlackLoader::new(config);

let docs = loader.load().await?;
for doc in &docs {
    println!("[{}] {}: {}", doc.metadata["channel"], doc.metadata["user"], doc.content);
}
```

## 配置选项

| 方法 | 说明 |
|------|------|
| `with_limit(n)` | 每个频道最多获取的消息数（默认：100） |
| `with_oldest(ts)` | 只获取该 Unix 时间戳之后的消息 |
| `with_threads()` | 包含线程回复（元数据中包含 `thread_ts` 字段） |

## 元数据字段

每个文档包含以下元数据：

- `source` — `slack:<channel-id>`
- `channel` — 频道 ID
- `ts` — 消息时间戳（Slack 格式，同时用作排序键）
- `user` — 发送者的 Slack 用户 ID
- `thread_ts` — 父线程时间戳（如消息属于某线程）
