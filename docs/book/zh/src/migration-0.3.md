# 迁移指南：0.2 → 0.3

本指南涵盖从 Synaptic 0.2 升级到 0.3 版本所需的全部变更。0.3 版本引入了**集成架构重构**，将 provider 拆分为独立 crate，并新增多个第三方集成。

## 概览

0.3 版本的主要变更：

1. **Provider crate 独立拆分** -- OpenAI、Anthropic、Gemini、Ollama 各自拥有独立 crate
2. **导入路径变更** -- provider 类型从 `synaptic::models` / `synaptic::embeddings` 迁移到各自的命名空间
3. **Feature flag 调整** -- 更细粒度的 provider 选择
4. **新增集成 crate** -- Qdrant、pgvector、Redis、PDF 等第三方集成
5. **核心 trait 整合** -- `Document`、`Retriever`、`VectorStore` 等 trait 迁移到 `synaptic-core`

---

## 1. Provider Crate 独立拆分

在 0.2 中，所有 LLM provider 适配器（OpenAI、Anthropic、Gemini、Ollama）都打包在 `synaptic-models` crate 中，embeddings 则在 `synaptic-embeddings` 中。

在 0.3 中，每个 provider 拥有独立的 crate：

| Provider | 新 crate | 包含内容 |
|----------|----------|----------|
| OpenAI | `synaptic-openai` | `OpenAiChatModel`、`OpenAiConfig`、`OpenAiEmbeddings`、`OpenAiEmbeddingsConfig` |
| Anthropic | `synaptic-anthropic` | `AnthropicChatModel`、`AnthropicConfig` |
| Gemini | `synaptic-gemini` | `GeminiChatModel`、`GeminiConfig` |
| Ollama | `synaptic-ollama` | `OllamaChatModel`、`OllamaConfig`、`OllamaEmbeddings`、`OllamaEmbeddingsConfig` |

`synaptic-models` 仍然保留，但只包含 provider 无关的通用类型：`ProviderBackend`、`HttpBackend`、`FakeBackend`、`ScriptedChatModel`、`RetryChatModel`、`RateLimitedChatModel`、`TokenBucketChatModel`、`StructuredOutputChatModel`、`BoundToolsChatModel`。

---

## 2. 导入路径变更

这是最常见的迁移操作。下面列出所有需要调整的导入路径。

### ChatModel 适配器

```rust
// 0.2（旧）
use synaptic::models::OpenAiChatModel;
use synaptic::models::OpenAiConfig;
use synaptic::models::AnthropicChatModel;
use synaptic::models::AnthropicConfig;
use synaptic::models::GeminiChatModel;
use synaptic::models::GeminiConfig;
use synaptic::models::OllamaChatModel;
use synaptic::models::OllamaConfig;

// 0.3（新）
use synaptic::openai::OpenAiChatModel;
use synaptic::openai::OpenAiConfig;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::anthropic::AnthropicConfig;
use synaptic::gemini::GeminiChatModel;
use synaptic::gemini::GeminiConfig;
use synaptic::ollama::OllamaChatModel;
use synaptic::ollama::OllamaConfig;
```

### Embeddings 适配器

```rust
// 0.2（旧）
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::embeddings::OpenAiEmbeddingsConfig;
use synaptic::embeddings::OllamaEmbeddings;
use synaptic::embeddings::OllamaEmbeddingsConfig;

// 0.3（新）
use synaptic::openai::OpenAiEmbeddings;
use synaptic::openai::OpenAiEmbeddingsConfig;
use synaptic::ollama::OllamaEmbeddings;
use synaptic::ollama::OllamaEmbeddingsConfig;
```

### 无需变更的导入

以下类型不受影响，导入路径保持不变：

```rust
// 这些保持不变
use synaptic::core::{ChatModel, Message, ChatRequest, ChatResponse};
use synaptic::core::{Embeddings, Document, Retriever, VectorStore};
use synaptic::models::ScriptedChatModel;
use synaptic::models::RetryChatModel;
use synaptic::models::ProviderBackend;
```

### 快速查找替换

对于大型代码库，可以使用以下命令批量替换：

```bash
# ChatModel 适配器
sed -i 's/synaptic::models::OpenAiChatModel/synaptic::openai::OpenAiChatModel/g' **/*.rs
sed -i 's/synaptic::models::OpenAiConfig/synaptic::openai::OpenAiConfig/g' **/*.rs
sed -i 's/synaptic::models::AnthropicChatModel/synaptic::anthropic::AnthropicChatModel/g' **/*.rs
sed -i 's/synaptic::models::AnthropicConfig/synaptic::anthropic::AnthropicConfig/g' **/*.rs
sed -i 's/synaptic::models::GeminiChatModel/synaptic::gemini::GeminiChatModel/g' **/*.rs
sed -i 's/synaptic::models::GeminiConfig/synaptic::gemini::GeminiConfig/g' **/*.rs
sed -i 's/synaptic::models::OllamaChatModel/synaptic::ollama::OllamaChatModel/g' **/*.rs
sed -i 's/synaptic::models::OllamaConfig/synaptic::ollama::OllamaConfig/g' **/*.rs

# Embeddings 适配器
sed -i 's/synaptic::embeddings::OpenAiEmbeddings/synaptic::openai::OpenAiEmbeddings/g' **/*.rs
sed -i 's/synaptic::embeddings::OpenAiEmbeddingsConfig/synaptic::openai::OpenAiEmbeddingsConfig/g' **/*.rs
sed -i 's/synaptic::embeddings::OllamaEmbeddings/synaptic::ollama::OllamaEmbeddings/g' **/*.rs
sed -i 's/synaptic::embeddings::OllamaEmbeddingsConfig/synaptic::ollama::OllamaEmbeddingsConfig/g' **/*.rs
```

---

## 3. Feature Flag 变更

### Cargo.toml 更新

```toml
# 0.2 -- "models" 拉取所有 provider
[dependencies]
synaptic = { version = "0.2", features = ["models"] }

# 0.3 -- 按需选择 provider（推荐）
[dependencies]
synaptic = { version = "0.3", features = ["openai"] }

# 0.3 -- 多个 provider
[dependencies]
synaptic = { version = "0.3", features = ["openai", "anthropic"] }

# 0.3 -- "models" 仍然可用，等同于启用全部 4 个 provider
[dependencies]
synaptic = { version = "0.3", features = ["models"] }
```

### Feature 对照表

| 0.2 Feature | 0.3 Feature | 说明 |
|-------------|-------------|------|
| `models` | `models` | 仍然可用，启用全部 4 个 provider |
| `models`（只需 OpenAI） | `openai` | 更精确，减少编译时间 |
| `models`（只需 Anthropic） | `anthropic` | 更精确，减少编译时间 |
| `models`（只需 Gemini） | `gemini` | 更精确，减少编译时间 |
| `models`（只需 Ollama） | `ollama` | 更精确，减少编译时间 |
| -- | `model-utils` | 只引入通用工具（`ScriptedChatModel`、`RetryChatModel` 等），不包含任何 provider |
| -- | `qdrant` | 新增：Qdrant 向量存储 |
| -- | `pgvector` | 新增：PostgreSQL pgvector 向量存储 |
| -- | `redis` | 新增：Redis 存储和缓存 |
| -- | `pdf` | 新增：PDF 文档加载器 |

### `synaptic::models` 模块内容变更

0.3 中 `synaptic::models` 不再包含 provider 适配器类型，只包含以下通用类型：

- `ProviderBackend`、`HttpBackend`、`FakeBackend` -- provider 后端抽象
- `ScriptedChatModel` -- 测试替身
- `RetryChatModel` -- 自动重试包装器
- `RateLimitedChatModel` -- 速率限制包装器
- `TokenBucketChatModel` -- 令牌桶限流包装器
- `StructuredOutputChatModel` -- 结构化输出包装器
- `BoundToolsChatModel` -- 工具绑定包装器

使用 `model-utils` feature 可以获取这些类型，而无需引入任何 provider。

---

## 4. 新增集成 Crate

0.3 新增了以下集成 crate，通过 feature flag 按需启用：

### Qdrant 向量存储

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant"] }
```

```rust
use synaptic::qdrant::QdrantVectorStore;
```

### PostgreSQL pgvector

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "pgvector"] }
```

```rust
use synaptic::pgvector::PgVectorStore;
```

### Redis 存储和缓存

```toml
[dependencies]
synaptic = { version = "0.3", features = ["redis"] }
```

```rust
use synaptic::redis::{RedisStore, RedisCache};
```

### PDF 文档加载器

```toml
[dependencies]
synaptic = { version = "0.3", features = ["pdf"] }
```

```rust
use synaptic::pdf::PdfLoader;
```

---

## 5. 核心 Trait 整合

以下 trait 和类型从原始 crate 迁移到了 `synaptic-core`：

| Trait/类型 | 原位置 | 新位置 |
|-----------|--------|--------|
| `Document` | `synaptic-retrieval` | `synaptic-core` |
| `Retriever` | `synaptic-retrieval` | `synaptic-core` |
| `VectorStore` | `synaptic-vectorstores` | `synaptic-core` |
| `Loader` | `synaptic-loaders` | `synaptic-core` |
| `LlmCache` | `synaptic-cache` | `synaptic-core` |

**向后兼容：** 原始 crate 中仍保留这些 trait 的再导出（re-export），因此现有的导入路径在 0.3 中仍然可用。但推荐从 `synaptic::core` 导入这些 trait：

```rust
// 推荐（0.3）
use synaptic::core::{Document, Retriever, VectorStore, Loader, LlmCache};

// 仍然可用（向后兼容），但不推荐
use synaptic::retrieval::Retriever;
use synaptic::vectorstores::VectorStore;
```

---

## 迁移步骤总结

按照以下步骤迁移你的项目：

### 步骤 1：更新 Cargo.toml

将版本号从 `0.2` 更新到 `0.3`，并按需调整 feature flag：

```toml
# 之前
synaptic = { version = "0.2", features = ["models", "graph"] }

# 之后 -- 只需要 OpenAI
synaptic = { version = "0.3", features = ["openai", "graph"] }

# 之后 -- 保留所有 provider
synaptic = { version = "0.3", features = ["models", "graph"] }
```

### 步骤 2：更新导入路径

将 provider 相关的导入从 `synaptic::models` / `synaptic::embeddings` 迁移到对应的 provider 命名空间：

```rust
// 之前
use synaptic::models::OpenAiChatModel;
use synaptic::embeddings::OpenAiEmbeddings;

// 之后
use synaptic::openai::OpenAiChatModel;
use synaptic::openai::OpenAiEmbeddings;
```

### 步骤 3：编译检查

```bash
cargo build --workspace
```

编译器会报告所有未解析的导入路径，按照上面的对照表逐一修复即可。

### 步骤 4（可选）：迁移核心 trait 导入

将 `Document`、`Retriever` 等 trait 的导入迁移到 `synaptic::core`。这一步不是必需的，因为原始路径仍然可用。

---

## 常见问题

### Q: `models` feature 还能用吗？

可以。`models` feature 在 0.3 中仍然可用，它会同时启用 `openai`、`anthropic`、`gemini`、`ollama` 四个 provider。如果你需要全部 provider，可以继续使用 `models`。

### Q: 我只用 OpenAI，需要改什么？

1. 将 `features = ["models"]` 改为 `features = ["openai"]`
2. 将 `use synaptic::models::OpenAiChatModel` 改为 `use synaptic::openai::OpenAiChatModel`
3. 将 `use synaptic::embeddings::OpenAiEmbeddings` 改为 `use synaptic::openai::OpenAiEmbeddings`

### Q: `ScriptedChatModel` 去哪了？

`ScriptedChatModel` 仍在 `synaptic::models` 中，使用 `model-utils` feature 即可获取，无需引入任何 provider。

### Q: 新增的集成 crate 是否有额外的系统依赖？

- **Qdrant** -- 需要运行 Qdrant 服务实例
- **pgvector** -- 需要安装了 pgvector 扩展的 PostgreSQL 数据库
- **Redis** -- 需要运行 Redis 服务实例
- **PDF** -- 无额外系统依赖

### Q: 这次迁移会破坏现有的 agent / graph 代码吗？

不会。`synaptic-graph`、`synaptic-tools`、`synaptic-memory` 等核心 crate 的 API 没有变化。只有 provider 的导入路径发生了变更。
