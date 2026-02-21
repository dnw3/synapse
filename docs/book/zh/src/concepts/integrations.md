# 集成

Synaptic 采用**以 Provider 为中心**的集成架构。每个集成位于独立的 crate 中，仅依赖 `synaptic-core`（加上对应的 provider SDK），并实现一个或多个核心 trait。

## 架构

```text
synaptic-core（定义 trait）
  ├── synaptic-openai     (ChatModel + Embeddings)
  ├── synaptic-anthropic  (ChatModel)
  ├── synaptic-gemini     (ChatModel)
  ├── synaptic-ollama     (ChatModel + Embeddings)
  ├── synaptic-qdrant     (VectorStore)
  ├── synaptic-pgvector   (VectorStore)
  ├── synaptic-redis      (Store + LlmCache)
  └── synaptic-pdf        (Loader)
```

所有集成 crate 遵循统一模式：

1. **核心 trait** — `ChatModel`、`Embeddings`、`VectorStore`、`Store`、`LlmCache`、`Loader` 定义在 `synaptic-core`
2. **独立 crate** — 每个集成是独立的 crate，拥有自己的 feature flag
3. **零耦合** — 集成 crate 之间互不依赖
4. **Config 结构体** — 使用 `new()` + `with_*()` 方法的 Builder 模式

## 核心 Trait

| Trait | 用途 | 实现 Crate |
|-------|------|-----------|
| `ChatModel` | LLM 聊天补全 | openai, anthropic, gemini, ollama |
| `Embeddings` | 文本嵌入向量 | openai, ollama |
| `VectorStore` | 向量相似度搜索 | qdrant, pgvector, (+ in-memory) |
| `Store` | 键值存储 | redis, (+ in-memory) |
| `LlmCache` | LLM 响应缓存 | redis, (+ in-memory) |
| `Loader` | 文档加载 | pdf, (+ text, json, csv, directory) |

## LLM Provider 模式

所有 LLM provider 遵循相同模式 — Config 结构体、Model 结构体，以及用于 HTTP 传输的 `ProviderBackend`：

```rust,ignore
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};
use synaptic::models::{HttpBackend, FakeBackend};

// 生产环境
let config = OpenAiConfig::new("sk-...", "gpt-4o");
let model = OpenAiChatModel::new(config, Arc::new(HttpBackend::new()));

// 测试（无网络调用）
let model = OpenAiChatModel::new(config, Arc::new(FakeBackend::with_responses(vec![...])));
```

`ProviderBackend` 抽象（位于 `synaptic-models`）提供：
- `HttpBackend` — 生产环境中的真实 HTTP 调用
- `FakeBackend` — 测试中的确定性响应

## 存储与检索模式

向量存储、键值存储和缓存实现核心 trait，支持即插即用的替换：

```rust,ignore
// 用 QdrantVectorStore 替换 InMemoryVectorStore — 相同的 trait 接口
use synaptic::qdrant::{QdrantVectorStore, QdrantConfig};

let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536);
let store = QdrantVectorStore::new(config);
store.add_documents(docs, &embeddings).await?;
let results = store.similarity_search("query", 5, &embeddings).await?;
```

## Feature Flags

每个集成在 `synaptic` facade crate 中拥有独立的 feature flag：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant"] }
```

| Feature | 集成 |
|---------|-----|
| `openai` | OpenAI ChatModel + Embeddings |
| `anthropic` | Anthropic ChatModel |
| `gemini` | Google Gemini ChatModel |
| `ollama` | Ollama ChatModel + Embeddings |
| `qdrant` | Qdrant 向量存储 |
| `pgvector` | PostgreSQL pgvector 存储 |
| `redis` | Redis 存储 + 缓存 |
| `pdf` | PDF 文档加载器 |

便捷组合：`models`（所有 LLM provider）、`agent`（包含 openai）、`rag`（包含 openai + 检索栈）、`full`（全部）。

## 添加新集成

添加新集成的步骤：

1. 在 `crates/` 下创建新 crate `synaptic-{name}`
2. 依赖 `synaptic-core` 获取 trait 定义
3. 实现相应的 trait
4. 在 `synaptic` facade crate 中添加 feature flag
5. 在 facade 的 `lib.rs` 中通过 `pub use synaptic_{name} as {name}` 再导出

## 另请参阅

- [安装](../installation.md) — Feature flag 参考
- [架构](architecture.md) — 整体系统设计
