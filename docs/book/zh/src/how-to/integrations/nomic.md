# Nomic AI

[Nomic AI](https://www.nomic.ai/) 提供开放权重的嵌入模型和免费 API 额度。`nomic-embed-text-v1.5` 模型支持 8192 token 上下文窗口，并为搜索、分类和聚类提供任务类型专用编码。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["nomic"] }
```

在 [atlas.nomic.ai](https://atlas.nomic.ai/) 获取免费 API 密钥。

## 使用示例

```rust,ignore
use synaptic::nomic::{NomicConfig, NomicEmbeddings};
use synaptic::core::Embeddings;

let config = NomicConfig::new("your-api-key");
let embeddings = NomicEmbeddings::new(config);

let docs = embeddings.embed_documents(&["长文档文本...", "另一个文档。"]).await?;
let query_vec = embeddings.embed_query("搜索查询").await?;
```

## 可用模型

| 枚举变体 | API 模型 ID | 上下文 | 说明 |
|---|---|---|---|
| `NomicEmbedTextV1_5` | `nomic-embed-text-v1.5` | 8192 token | 默认，最高质量 |
| `NomicEmbedTextV1` | `nomic-embed-text-v1` | 2048 token | 旧版 |

## 任务类型

Nomic 使用任务类型专用编码。`embed_documents()` 自动使用 `search_document`，`embed_query()` 自动使用 `search_query`。
