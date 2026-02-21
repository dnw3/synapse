# 第三方集成

Synaptic 通过可选的 feature flag 提供与外部服务和数据源的集成。每个集成都封装在独立的 crate 中，实现 Synaptic 核心 trait，可以直接与现有的检索、缓存和 Agent 流水线配合使用。

## 可用集成

### LLM Provider

| 集成 | Feature | 说明 |
|------|---------|------|
| [OpenAI 兼容 Provider](openai-compatible.md) | `openai` | 9 个内置便捷构造器（Groq、DeepSeek、Fireworks、Together、xAI、MistralAI、HuggingFace、Cohere、OpenRouter） |
| [Azure OpenAI](azure-openai.md) | `openai` | Azure OpenAI Service（基于 deployment 的 URL + `api-key` 认证） |
| [AWS Bedrock](bedrock.md) | `bedrock` | AWS Bedrock 托管模型（Claude、Llama、Mistral 等） |

### 向量存储

| 集成 | Feature | 实现的 Trait | 用途 |
|------|---------|-------------|------|
| [Qdrant](qdrant.md) | `qdrant` | `VectorStore` | 高性能向量数据库，支持分布式部署和多种距离度量 |
| [pgvector](pgvector.md) | `pgvector` | `VectorStore` | 基于 PostgreSQL 的向量存储，利用 pgvector 扩展实现相似性搜索 |
| [Pinecone](pinecone.md) | `pinecone` | `VectorStore` | 全托管向量数据库，专为大规模相似性搜索设计 |
| [Chroma](chroma.md) | `chroma` | `VectorStore` | 开源嵌入数据库，支持本地和云端部署 |
| [MongoDB Atlas](mongodb.md) | `mongodb` | `VectorStore` | MongoDB Atlas 原生向量搜索，在现有 MongoDB 上启用向量检索 |
| [Elasticsearch](elasticsearch.md) | `elasticsearch` | `VectorStore` | Elasticsearch kNN 向量搜索，利用 dense vector 字段 |

### 存储与缓存

| 集成 | Feature | 实现的 Trait | 用途 |
|------|---------|-------------|------|
| [Redis](redis.md) | `redis` | `Store` + `LlmCache` | Redis 键值存储和 LLM 响应缓存，支持 TTL 和前缀隔离 |
| [SQLite](sqlite.md) | `sqlite` | `LlmCache` | 基于 SQLite 的持久化 LLM 缓存，无需外部服务器 |

### 文档加载

| 集成 | Feature | 实现的 Trait | 用途 |
|------|---------|-------------|------|
| [PDF](pdf.md) | `pdf` | `Loader` | PDF 文档加载器，支持整文档或按页拆分加载 |

### 检索增强

| 集成 | Feature | 说明 |
|------|---------|------|
| [Cohere Reranker](cohere.md) | `cohere` | 使用 Cohere 重排序模型对检索结果精排，实现 `DocumentCompressor` |

### 工具

| 集成 | Feature | 实现的 Trait | 用途 |
|------|---------|-------------|------|
| [Tavily](tavily.md) | `tavily` | `Tool` | AI 优化的网络搜索工具 |

## 启用集成

在 `Cargo.toml` 中通过 feature flag 启用所需的集成：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["qdrant", "pinecone", "redis", "tavily"] }
```

你可以只启用需要的集成，无需全部引入。每个集成只会引入它自身所需的依赖。

## 与核心组件的关系

所有集成都实现了 Synaptic 核心 trait，因此可以无缝替换内置实现：

- **Qdrant / pgvector / Pinecone / Chroma / MongoDB / Elasticsearch** 替代 `InMemoryVectorStore` -- 提供持久化和可扩展的向量存储
- **Redis Store** 替代 `InMemoryStore` -- 提供跨进程共享的键值存储
- **Redis Cache / SQLite Cache** 替代 `InMemoryCache` -- 提供持久化的 LLM 响应缓存
- **PDF Loader** 补充现有的 `TextLoader`、`JsonLoader` 等 -- 增加 PDF 格式支持
- **Cohere Reranker** 与 `ContextualCompressionRetriever` 配合 -- 提升检索精度
- **Tavily** 作为 Agent 工具 -- 为 Agent 添加网络搜索能力

## 指南

- [OpenAI 兼容 Provider](openai-compatible.md) -- 使用 Groq、DeepSeek 等 OpenAI 兼容 API
- [Azure OpenAI](azure-openai.md) -- 接入 Azure OpenAI Service
- [AWS Bedrock](bedrock.md) -- 接入 AWS Bedrock 托管模型
- [Cohere Reranker](cohere.md) -- 使用 Cohere 重排序模型
- [Qdrant 向量存储](qdrant.md) -- 使用 Qdrant 存储和搜索嵌入向量
- [PgVector](pgvector.md) -- 使用 PostgreSQL + pgvector 存储和搜索嵌入向量
- [Pinecone 向量存储](pinecone.md) -- 使用 Pinecone 全托管向量数据库
- [Chroma 向量存储](chroma.md) -- 使用 Chroma 开源嵌入数据库
- [MongoDB Atlas 向量搜索](mongodb.md) -- 使用 MongoDB Atlas 原生向量搜索
- [Elasticsearch 向量存储](elasticsearch.md) -- 使用 Elasticsearch kNN 搜索
- [Redis 存储与缓存](redis.md) -- 使用 Redis 进行持久化存储和缓存
- [SQLite 缓存](sqlite.md) -- 使用 SQLite 进行本地 LLM 缓存
- [PDF 加载器](pdf.md) -- 从 PDF 文件加载文档
- [Tavily 搜索工具](tavily.md) -- 为 Agent 添加网络搜索能力
