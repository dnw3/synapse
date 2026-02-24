# 集成

Synaptic 采用**以 Provider 为中心**的集成架构。每个集成位于独立的 crate 中，仅依赖 `synaptic-core`（加上对应的 provider SDK），并实现一个或多个核心 trait。

## 架构

```text
synaptic-core（定义 trait）
  ├── synaptic-openai         (ChatModel + Embeddings)
  ├── synaptic-anthropic      (ChatModel)
  ├── synaptic-gemini         (ChatModel)
  ├── synaptic-ollama         (ChatModel + Embeddings)
  ├── synaptic-bedrock        (ChatModel)
  ├── synaptic-groq           (ChatModel — OpenAI 兼容，LPU)
  ├── synaptic-mistral        (ChatModel — OpenAI 兼容)
  ├── synaptic-deepseek       (ChatModel — OpenAI 兼容)
  ├── synaptic-cohere         (Reranker / DocumentCompressor + Embeddings)
  ├── synaptic-huggingface    (Embeddings)
  ├── synaptic-qdrant         (VectorStore)
  ├── synaptic-postgres       (VectorStore + Store + LlmCache + Checkpointer)
  ├── synaptic-pinecone       (VectorStore)
  ├── synaptic-chroma         (VectorStore)
  ├── synaptic-mongodb        (VectorStore)
  ├── synaptic-elasticsearch  (VectorStore)
  ├── synaptic-weaviate       (VectorStore)
  ├── synaptic-redis          (Store + LlmCache + Checkpointer)
  ├── synaptic-sqlite         (LlmCache)
  ├── synaptic-pdf            (Loader)
  ├── synaptic-tavily         (Tool)
  └── synaptic-sqltoolkit     (Tool×3: ListTables, DescribeTable, ExecuteQuery)
```

所有集成 crate 遵循统一模式：

1. **核心 trait** — `ChatModel`、`Embeddings`、`VectorStore`、`Store`、`LlmCache`、`Loader`、`Tool`、`DocumentCompressor` 定义在 `synaptic-core`
2. **独立 crate** — 每个集成是独立的 crate，拥有自己的 feature flag
3. **零耦合** — 集成 crate 之间互不依赖
4. **Config 结构体** — 使用 `new()` + `with_*()` 方法的 Builder 模式

## 核心 Trait

| Trait | 用途 | 实现 Crate |
|-------|------|-----------|
| `ChatModel` | LLM 聊天补全 | openai, anthropic, gemini, ollama, bedrock, groq, mistral, deepseek |
| `Embeddings` | 文本嵌入向量 | openai, ollama, cohere, huggingface |
| `VectorStore` | 向量相似度搜索 | qdrant, postgres, pinecone, chroma, mongodb, elasticsearch, weaviate, (+ in-memory) |
| `Store` | 键值存储 | redis, postgres, (+ in-memory) |
| `LlmCache` | LLM 响应缓存 | redis, sqlite, postgres, (+ in-memory) |
| `Checkpointer` | Graph 状态持久化 | redis, postgres |
| `Loader` | 文档加载 | pdf, (+ text, json, csv, directory) |
| `Tool` | Agent 工具 | tavily, sqltoolkit（3 个工具）, duckduckgo, wikipedia, (+ 自定义工具) |
| `DocumentCompressor` | 文档压缩/重排序 | cohere, (+ embeddings-filter) |

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

> **注意：** AWS Bedrock 是例外，它直接使用 AWS SDK 而非 `ProviderBackend`。

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
synaptic = { version = "0.2", features = ["openai", "qdrant"] }
```

| Feature | 集成 |
|---------|-----|
| `openai` | OpenAI ChatModel + Embeddings |
| `anthropic` | Anthropic ChatModel |
| `gemini` | Google Gemini ChatModel |
| `ollama` | Ollama ChatModel + Embeddings |
| `bedrock` | AWS Bedrock ChatModel |
| `groq` | Groq ChatModel（超快 LPU 推理，OpenAI 兼容） |
| `mistral` | Mistral ChatModel（OpenAI 兼容） |
| `deepseek` | DeepSeek ChatModel（成本极低，OpenAI 兼容） |
| `cohere` | Cohere Reranker + Embeddings |
| `huggingface` | HuggingFace Inference API Embeddings |
| `qdrant` | Qdrant 向量存储 |
| `postgres` | PostgreSQL 集成（VectorStore + Store + Cache + 检查点） |
| `pinecone` | Pinecone 向量存储 |
| `chroma` | Chroma 向量存储 |
| `mongodb` | MongoDB Atlas 向量搜索 |
| `elasticsearch` | Elasticsearch 向量存储 |
| `weaviate` | Weaviate 向量存储 |
| `redis` | Redis 存储 + 缓存 + Graph 检查点 |
| `sqlite` | SQLite LLM 缓存 |
| `pdf` | PDF 文档加载器 |
| `tavily` | Tavily 搜索工具 |
| `sqltoolkit` | SQL 数据库工具包（ListTables、DescribeTable、ExecuteQuery） |

便捷组合：`models`（全部 9 个 LLM provider）、`agent`（包含 openai + graph）、`rag`（包含 openai + 检索栈）、`full`（全部）。

## Provider 选型指南

根据需求选择合适的 LLM Provider：

| Provider | 认证方式 | 流式传输 | 工具调用 | 嵌入 | 适用场景 |
|----------|---------|---------|---------|------|---------|
| **OpenAI** | API key (Header) | SSE | 支持 | 支持 | 通用场景，模型选择最广 |
| **Anthropic** | API key (`x-api-key`) | SSE | 支持 | 不支持 | 长上下文、推理任务 |
| **Gemini** | API key (查询参数) | SSE | 支持 | 不支持 | Google 生态、多模态 |
| **Ollama** | 无需认证（本地） | NDJSON | 支持 | 支持 | 隐私敏感、离线、开发调试 |
| **Bedrock** | AWS IAM | AWS SDK | 支持 | 不支持 | 企业 AWS 环境 |
| **Groq** | API key (Header) | SSE | 支持 | 不支持 | 超快推理（LPU），延迟敏感场景 |
| **Mistral** | API key (Header) | SSE | 支持 | 不支持 | 欧盟合规，低成本工具调用 |
| **DeepSeek** | API key (Header) | SSE | 支持 | 不支持 | 成本极低（比 GPT-4o 便宜 90%+），国内首选 |
| **Cohere** | API key (Header) | — | — | 支持 | Reranker + 生产级嵌入 |
| **HuggingFace** | API key（可选） | — | — | 支持 | 开源 sentence-transformers |

**决策因素：**

- **隐私合规** — Ollama 完全本地运行；Bedrock 数据不出 AWS
- **成本** — Ollama 免费；OpenAI 兼容 Provider（Groq、DeepSeek）价格有竞争力
- **延迟** — Ollama 无网络往返；Groq 针对速度优化
- **生态** — OpenAI 第三方集成最丰富；Bedrock 与 AWS 服务深度集成

## 向量数据库选型指南

| 存储 | 部署方式 | 托管服务 | 筛选能力 | 扩展性 | 适用场景 |
|------|---------|---------|---------|-------|---------|
| **Qdrant** | 自托管 / 云 | Qdrant Cloud | 丰富（payload 过滤） | 水平扩展 | 通用场景，生产环境 |
| **pgvector** | 自托管 | 托管 PostgreSQL | SQL WHERE | 垂直扩展 | 已有 PostgreSQL 的团队 |
| **Pinecone** | 全托管 | 内置 | 元数据过滤 | 自动扩展 | 零运维，快速原型 |
| **Chroma** | 自托管 / Docker | 无 | 元数据过滤 | 单节点 | 开发环境，中小数据集 |
| **MongoDB Atlas** | 全托管 | 内置 | MQL 过滤 | 自动扩展 | 已有 MongoDB 的团队 |
| **Elasticsearch** | 自托管 / 云 | Elastic Cloud | 完整查询 DSL | 水平扩展 | 混合文本 + 向量搜索 |
| **Weaviate** | 自托管 / 云 | WCS | GraphQL 过滤 | 水平扩展 | 多租户、混合搜索 |
| **InMemory** | 进程内 | 不适用 | 无 | 不适用 | 测试、原型验证 |

**决策因素：**

- **现有基础设施** — 已有 PostgreSQL 用 pgvector，已有 MongoDB 用 Atlas，已有 ES 集群用 Elasticsearch
- **运维复杂度** — Pinecone 和 MongoDB Atlas 全托管；Qdrant 和 Elasticsearch 需要集群管理
- **查询能力** — Elasticsearch 擅长混合文本 + 向量查询；Qdrant 过滤能力最丰富
- **成本** — InMemory 和 Chroma 免费；pgvector 复用现有数据库基础设施

## 缓存选型指南

| 缓存 | 持久化 | 部署方式 | TTL 支持 | 适用场景 |
|------|-------|---------|---------|---------|
| **InMemory** | 否（进程生命周期） | 进程内 | 支持 | 测试、单进程应用 |
| **Redis** | 是（可配置） | 外部服务 | 支持 | 多进程、分布式 |
| **SQLite** | 是（文件） | 进程内 | 支持 | 单机持久化 |
| **Semantic** | 取决于底层存储 | 进程内 | 不支持 | 模糊匹配缓存 |

## 完整 RAG 流水线示例

以下示例将多个集成组合成完整的检索增强生成流水线，包含缓存和重排序：

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message, Embeddings};
use synaptic::openai::{OpenAiChatModel, OpenAiConfig, OpenAiEmbeddings};
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};
use synaptic::cohere::{CohereReranker, CohereConfig};
use synaptic::cache::{CachedChatModel, InMemoryCache};
use synaptic::retrieval::ContextualCompressionRetriever;
use synaptic::splitters::RecursiveCharacterTextSplitter;
use synaptic::loaders::TextLoader;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::models::HttpBackend;
use std::sync::Arc;

let backend = Arc::new(HttpBackend::new());

// 1. 配置嵌入模型
let embeddings = Arc::new(OpenAiEmbeddings::new(
    OpenAiEmbeddings::config("text-embedding-3-small"),
    backend.clone(),
));

// 2. 将文档导入 Qdrant
let loader = TextLoader::new("knowledge-base.txt");
let docs = loader.load().await?;
let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&docs)?;

let qdrant_config = QdrantConfig::new("http://localhost:6334", "knowledge", 1536);
let store = QdrantVectorStore::new(qdrant_config, embeddings.clone()).await?;
store.add_documents(&chunks).await?;

// 3. 构建带 Cohere 重排序的检索器
let base_retriever = Arc::new(VectorStoreRetriever::new(Arc::new(store)));
let reranker = CohereReranker::new(CohereConfig::new(std::env::var("COHERE_API_KEY")?));
let retriever = ContextualCompressionRetriever::new(base_retriever, Arc::new(reranker));

// 4. 用缓存包装 LLM
let llm_config = OpenAiConfig::new(std::env::var("OPENAI_API_KEY")?, "gpt-4o");
let base_model = OpenAiChatModel::new(llm_config, backend.clone());
let cache = Arc::new(InMemoryCache::new());
let model = CachedChatModel::new(Arc::new(base_model), cache);

// 5. 检索并生成回答
let relevant = retriever.retrieve("Synaptic 如何处理流式传输？").await?;
let context = relevant.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join("\n\n");

let request = ChatRequest::new(vec![
    Message::system(&format!("根据以下上下文回答问题：\n\n{context}")),
    Message::human("Synaptic 如何处理流式传输？"),
]);
let response = model.chat(&request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

此流水线演示了：
- **Qdrant** 用于向量存储和检索
- **Cohere** 用于重排序检索结果
- **InMemoryCache** 用于缓存 LLM 响应（可替换为 Redis/SQLite 实现持久化）
- **OpenAI** 同时提供嵌入和聊天补全

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
