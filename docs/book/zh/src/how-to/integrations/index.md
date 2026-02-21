# 第三方集成

Synaptic 通过可选的 feature flag 提供与外部存储和数据源的集成。每个集成都封装在独立的 crate 中，实现 Synaptic 核心 trait，可以直接与现有的检索、缓存和 Agent 流水线配合使用。

## 可用集成

| 集成 | Feature | 实现的 Trait | 用途 |
|------|---------|-------------|------|
| [Qdrant](qdrant.md) | `qdrant` | `VectorStore` | 高性能向量数据库，支持分布式部署和多种距离度量 |
| [pgvector](pgvector.md) | `pgvector` | `VectorStore` | 基于 PostgreSQL 的向量存储，利用 pgvector 扩展实现相似性搜索 |
| [Redis](redis.md) | `redis` | `Store` + `LlmCache` | Redis 键值存储和 LLM 响应缓存，支持 TTL 和前缀隔离 |
| [PDF](pdf.md) | `pdf` | `Loader` | PDF 文档加载器，支持整文档或按页拆分加载 |

## 启用集成

在 `Cargo.toml` 中通过 feature flag 启用所需的集成：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["qdrant", "pgvector", "redis", "pdf"] }
```

你可以只启用需要的集成，无需全部引入。

## 与核心组件的关系

所有集成都实现了 Synaptic 核心 trait（`VectorStore`、`Store`、`LlmCache`、`Loader`），因此可以无缝替换内置实现：

- **Qdrant / pgvector** 替代 `InMemoryVectorStore` -- 提供持久化和可扩展的向量存储
- **Redis Store** 替代 `InMemoryStore` -- 提供跨进程共享的键值存储
- **Redis Cache** 替代 `InMemoryCache` -- 提供持久化的 LLM 响应缓存
- **PDF Loader** 补充现有的 `TextLoader`、`JsonLoader` 等 -- 增加 PDF 格式支持
