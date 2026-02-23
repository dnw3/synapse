# API 参考

Synaptic 的完整 API 文档由 `rustdoc` 生成。你可以在本地构建并浏览：

```bash
cargo doc --workspace --open
```

此命令会为所有 crate 生成文档并在浏览器中打开。

## Crate 一览

下表列出了 Synaptic 的所有 crate 及其职责：

| Crate | 说明 |
|---|---|
| [`synaptic`](https://docs.rs/synaptic) | 统一 facade crate，重新导出所有子 crate |
| [`synaptic-core`](https://docs.rs/synaptic-core) | 核心 trait 和类型：`ChatModel`、`Message`、`Tool`、`SynapticError`、`RunnableConfig` 等 |
| [`synaptic-models`](https://docs.rs/synaptic-models) | `ProviderBackend` 抽象、`ScriptedChatModel` 测试替身、ChatModel 包装器（重试、速率限制、结构化输出、BoundTools） |
| [`synaptic-prompts`](https://docs.rs/synaptic-prompts) | 提示模板：`PromptTemplate`、`ChatPromptTemplate`、`FewShotChatMessagePromptTemplate` |
| [`synaptic-parsers`](https://docs.rs/synaptic-parsers) | 输出解析器：`StrOutputParser`、`JsonOutputParser`、`StructuredOutputParser`、`ListOutputParser`、`EnumOutputParser` 等 |
| [`synaptic-runnables`](https://docs.rs/synaptic-runnables) | LCEL 组合原语：`Runnable` trait、`BoxRunnable`、管道运算符、`RunnableParallel`、`RunnableBranch` 等 |
| [`synaptic-tools`](https://docs.rs/synaptic-tools) | 工具注册表（`ToolRegistry`）和串行执行器（`SerialToolExecutor`） |
| [`synaptic-memory`](https://docs.rs/synaptic-memory) | 会话记忆策略：Buffer、Window、Summary、Token Buffer、Summary Buffer |
| [`synaptic-graph`](https://docs.rs/synaptic-graph) | LangGraph 风格状态机：`StateGraph`、`CompiledGraph`、`ToolNode`、`create_react_agent` |
| [`synaptic-callbacks`](https://docs.rs/synaptic-callbacks) | 回调处理器：`RecordingCallback`、`TracingCallback`、`CompositeCallback` |
| [`synaptic-cache`](https://docs.rs/synaptic-cache) | LLM 缓存：`InMemoryCache`（可选 TTL）、`SemanticCache`（嵌入相似度匹配） |
| [`synaptic-loaders`](https://docs.rs/synaptic-loaders) | 文档加载器：`TextLoader`、`JsonLoader`、`CsvLoader`、`DirectoryLoader` |
| [`synaptic-splitters`](https://docs.rs/synaptic-splitters) | 文本分割器：`CharacterTextSplitter`、`RecursiveCharacterTextSplitter`、`MarkdownHeaderTextSplitter`、`TokenTextSplitter` |
| [`synaptic-embeddings`](https://docs.rs/synaptic-embeddings) | 嵌入模型：`Embeddings` trait、`FakeEmbeddings`、`CacheBackedEmbeddings` |
| [`synaptic-vectorstores`](https://docs.rs/synaptic-vectorstores) | 向量存储：`InMemoryVectorStore`（cosine 相似度）、`VectorStoreRetriever` |
| [`synaptic-retrieval`](https://docs.rs/synaptic-retrieval) | 检索器：`BM25Retriever`、`MultiQueryRetriever`、`EnsembleRetriever`、`SelfQueryRetriever`、`ParentDocumentRetriever` 等 |
| [`synaptic-openai`](https://docs.rs/synaptic-openai) | OpenAI 提供商：`OpenAiChatModel`、`OpenAiEmbeddings` |
| [`synaptic-anthropic`](https://docs.rs/synaptic-anthropic) | Anthropic 提供商：`AnthropicChatModel` |
| [`synaptic-gemini`](https://docs.rs/synaptic-gemini) | Google Gemini 提供商：`GeminiChatModel` |
| [`synaptic-ollama`](https://docs.rs/synaptic-ollama) | Ollama 提供商：`OllamaChatModel`、`OllamaEmbeddings` |
| [`synaptic-qdrant`](https://docs.rs/synaptic-qdrant) | Qdrant 向量存储：`QdrantVectorStore` |
| [`synaptic-pgvector`](https://docs.rs/synaptic-pgvector) | PostgreSQL pgvector 向量存储：`PgVectorStore` |
| [`synaptic-redis`](https://docs.rs/synaptic-redis) | Redis 存储和缓存：`RedisStore`、`RedisCache` |
| [`synaptic-pdf`](https://docs.rs/synaptic-pdf) | PDF 文档加载器：`PdfLoader` |
| [`synaptic-eval`](https://docs.rs/synaptic-eval) | 评估器：`ExactMatchEvaluator`、`RegexMatchEvaluator`、`LLMJudgeEvaluator` 等 |

## 常用导入

使用 `synaptic` facade crate 时的常用导入路径：

```rust
// 核心类型
use synaptic::core::{ChatModel, Message, ChatRequest, ChatResponse, SynapticError};
use synaptic::core::{Tool, ToolCall, ToolChoice, ToolDefinition};
use synaptic::core::{RunnableConfig, TokenUsage, RunEvent};
use synaptic::core::{AIMessageChunk, ChatStream};

// 提供商模型
use synaptic::openai::OpenAiChatModel;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::gemini::GeminiChatModel;
use synaptic::ollama::OllamaChatModel;

// 模型工具
use synaptic::models::{ScriptedChatModel, RetryChatModel, RateLimitedChatModel};
use synaptic::models::StructuredOutputChatModel;

// Runnables
use synaptic::runnables::{Runnable, BoxRunnable, RunnableLambda};
use synaptic::runnables::{RunnableParallel, RunnableBranch, RunnablePassthrough};
use synaptic::runnables::{RunnableAssign, RunnablePick, RunnableWithFallbacks};

// Prompts
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic::prompts::FewShotChatMessagePromptTemplate;

// Parsers
use synaptic::parsers::{StrOutputParser, JsonOutputParser, StructuredOutputParser};

// Graph
use synaptic::graph::{StateGraph, CompiledGraph, MessageState, ToolNode};
use synaptic::graph::{create_react_agent, StreamMode, GraphEvent};
use synaptic::graph::StoreCheckpointer;

// Retrieval
use synaptic::retrieval::{Retriever, InMemoryRetriever, BM25Retriever};
use synaptic::vectorstores::{InMemoryVectorStore, VectorStoreRetriever};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::openai::OpenAiEmbeddings;
```

## 构建文档

```bash
# 构建所有 crate 的文档
cargo doc --workspace --open

# 构建单个 crate 的文档
cargo doc -p synaptic-core --open

# 包含私有项的文档（开发者参考）
cargo doc --workspace --open
```
