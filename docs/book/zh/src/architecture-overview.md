# 架构概览

Synaptic 采用 Cargo workspace 组织，包含 22 个库 crate、1 个门面 crate 和若干示例二进制文件。这些 crate 形成分层架构，每一层建立在下一层之上。

## Crate 层级

### 核心层

**`synaptic-core`** 定义所有共享的 trait 和类型。所有其他 crate 都依赖它。

- Trait：`ChatModel`、`Tool`、`RuntimeAwareTool`、`MemoryStore`、`CallbackHandler`、`Store`、`Embeddings`
- 类型：`Message`、`ChatRequest`、`ChatResponse`、`ToolCall`、`ToolDefinition`、`ToolChoice`、`AIMessageChunk`、`TokenUsage`、`RunEvent`、`RunnableConfig`、`Runtime`、`ToolRuntime`、`ModelProfile`、`Item`、`ContentBlock`
- 错误类型：`SynapticError`（20 个变体，覆盖所有子系统）
- 流类型：`ChatStream`（`Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send>>`）

### 实现层

每个 crate 实现一个核心 trait 或提供一项专注的能力：

| Crate | 职责 |
|---|---|
| `synaptic-models` | `ProviderBackend` 抽象、`ScriptedChatModel` 测试替身、ChatModel 包装器（重试、速率限制、缓存、结构化输出、BoundTools） |
| `synaptic-tools` | `ToolRegistry`、`SerialToolExecutor`、`ParallelToolExecutor` |
| `synaptic-memory` | 记忆策略：buffer、window、summary、token buffer、summary buffer、`RunnableWithMessageHistory` |
| `synaptic-callbacks` | `RecordingCallback`、`TracingCallback`、`CompositeCallback` |
| `synaptic-prompts` | `PromptTemplate`、`ChatPromptTemplate`、`FewShotChatMessagePromptTemplate` |
| `synaptic-parsers` | 输出解析器：string、JSON、structured、list、enum、boolean、XML、markdown list、numbered list |
| `synaptic-cache` | `InMemoryCache`、`SemanticCache`、`CachedChatModel` |

### 提供商层

每个 LLM 提供商由独立的 crate 提供，可按需启用：

| Crate | 职责 |
|---|---|
| `synaptic-openai` | `OpenAiChatModel`、`OpenAiEmbeddings` |
| `synaptic-anthropic` | `AnthropicChatModel` |
| `synaptic-gemini` | `GeminiChatModel` |
| `synaptic-ollama` | `OllamaChatModel`、`OllamaEmbeddings` |

### 组合层

这些 crate 提供更高层次的编排功能：

| Crate | 职责 |
|---|---|
| `synaptic-runnables` | `Runnable` trait，含 `invoke()`/`batch()`/`stream()`，`BoxRunnable` 管道运算符，`RunnableLambda`、`RunnableParallel`、`RunnableBranch`、`RunnableAssign`、`RunnablePick`、`RunnableWithFallbacks` |
| `synaptic-graph` | LangGraph 风格状态机：`StateGraph`、`CompiledGraph`、`ToolNode`、`create_react_agent`、`create_supervisor`、`create_swarm`、`Command`、`GraphResult`、`Checkpointer`、`StoreCheckpointer`、多模式流 |

### 检索流水线

这些 crate 构成文档摄取和检索流水线：

| Crate | 职责 |
|---|---|
| `synaptic-loaders` | `TextLoader`、`JsonLoader`、`CsvLoader`、`DirectoryLoader` |
| `synaptic-splitters` | `CharacterTextSplitter`、`RecursiveCharacterTextSplitter`、`MarkdownHeaderTextSplitter`、`TokenTextSplitter` |
| `synaptic-embeddings` | `Embeddings` trait、`FakeEmbeddings`、`CacheBackedEmbeddings` |
| `synaptic-vectorstores` | `VectorStore` trait、`InMemoryVectorStore`、`VectorStoreRetriever` |
| `synaptic-retrieval` | `Retriever` trait、`BM25Retriever`、`MultiQueryRetriever`、`EnsembleRetriever`、`ContextualCompressionRetriever`、`SelfQueryRetriever`、`ParentDocumentRetriever` |

### 评估层

| Crate | 职责 |
|---|---|
| `synaptic-eval` | `Evaluator` trait、`ExactMatchEvaluator`、`RegexMatchEvaluator`、`JsonValidityEvaluator`、`EmbeddingDistanceEvaluator`、`LLMJudgeEvaluator`、`Dataset`、批量评估流水线 |

### 高级 Crate

这些 crate 为生产级 agent 系统提供专业能力：

| Crate | 职责 |
|---|---|
| `synaptic-store` | `Store` trait 实现、带语义搜索的 `InMemoryStore`（可选嵌入向量） |
| `synaptic-middleware` | `AgentMiddleware` trait、`MiddlewareChain`、内置中间件：模型重试、PII 过滤、提示缓存、摘要、人工审批、工具调用限制 |
| `synaptic-mcp` | Model Context Protocol 适配器：`MultiServerMcpClient`、Stdio/SSE/HTTP 传输层，用于工具发现和调用 |
| `synaptic-macros` | 过程宏：`#[tool]`、`#[chain]`、`#[entrypoint]`、`#[task]`、`#[traceable]`、中间件宏 |
| `synaptic-deep` | Deep Agent 运行框架：`Backend` trait（State/Store/Filesystem）、7 个文件系统工具、6 个中间件、`create_deep_agent()` 工厂函数 |

### 集成层

这些 crate 提供外部系统的集成：

| Crate | 职责 |
|---|---|
| `synaptic-qdrant` | Qdrant 向量存储（`QdrantVectorStore`） |
| `synaptic-pgvector` | PostgreSQL pgvector 向量存储（`PgVectorStore`） |
| `synaptic-redis` | Redis 存储和缓存（`RedisStore`、`RedisCache`） |
| `synaptic-pdf` | PDF 文档加载器（`PdfLoader`） |

### 门面层

**`synaptic`** 重新导出所有子 crate，提供便捷的单一导入方式：

```rust
use synaptic::core::{ChatModel, Message, ChatRequest};
use synaptic::openai::OpenAiChatModel;
use synaptic::runnables::{Runnable, RunnableLambda};
use synaptic::graph::{StateGraph, create_react_agent};
```

## 依赖关系图

所有 crate 依赖 `synaptic-core` 获取共享的 trait 和类型。高层 crate 依赖其下方的层级：

```text
                            ┌──────────┐
                            │ synaptic │  (facade: re-exports all)
                            └─────┬────┘
                                  │
     ┌──────────────┬─────────────┼──────────────┬───────────────┐
     │              │             │              │               │
 ┌───┴───┐   ┌─────┴────┐  ┌────┴─────┐  ┌─────┴────┐   ┌─────┴───┐
 │ deep  │   │middleware│  │  graph   │  │runnables │   │  eval   │
 └───┬───┘   └─────┬────┘  └────┬─────┘  └────┬─────┘   └─────┬───┘
     │              │            │              │               │
     ├──────────────┴────┬───────┴──────────────┤               │
     │                   │                      │               │
┌────┴──┐ ┌─────┐ ┌─────┴──┐ ┌──────┐ ┌───────┐│┌──────┐┌─────┴──┐
│models │ │tools│ │memory  │ │store │ │prompts│││parsers││cache   │
└───┬───┘ └──┬──┘ └───┬────┘ └──┬───┘ └───┬───┘│└───┬───┘└───┬────┘
    │        │        │         │         │    │    │        │
    │  ┌─────┴─┬──────┤    ┌────┘         │    │    │        │
    │  │       │      │    │              │    │    │        │
    ├──┤  ┌────┴──┐   │  ┌─┴────┐  ┌─────┴────┴────┴────────┤
    │  │  │macros │   │  │ mcp  │  │    callbacks            │
    │  │  └───┬───┘   │  └──┬───┘  └────────┬────────────────┘
    │  │      │       │     │               │
  ┌─┴──┴──────┴───────┴─────┴───────────────┴──┐
  │              synaptic-core                  │
  │  (ChatModel, Tool, Store, Embeddings, ...) │
  └─────────────────────────────────────────────┘

  Retrieval pipeline:

  loaders ──► splitters ──► embeddings ──► vectorstores ──► retrieval
                                                              │
                                                        synaptic-core
```

## 设计原则

### 异步优先与 `#[async_trait]`

Synaptic 中的每个 trait 都是异步的。`ChatModel::chat()` 方法、`Tool::call()`、`MemoryStore::load()` 和 `Runnable::invoke()` 都是异步函数。这意味着你可以在任何实现中自由 `await` 网络调用、数据库查询和并发操作，而不会阻塞运行时。

### 基于 `Arc` 的共享

Synaptic 对注册表（如 `ToolRegistry`）使用 `Arc<RwLock<_>>`，允许多个读取者并发访问；对有状态组件（如回调和记忆存储）使用 `Arc<tokio::sync::Mutex<_>>`，确保修改操作串行化。这允许在异步任务和 agent 会话间安全共享。

### 会话隔离

记忆存储和 agent 运行通过 `session_id` 进行键值隔离。多个对话可以在同一模型和工具集上并发运行，状态不会在会话间泄漏。

### 事件驱动的回调

`CallbackHandler` trait 在每个生命周期阶段接收 `RunEvent` 值（运行开始、LLM 调用、工具调用、运行完成、运行失败）。你可以使用 `CompositeCallback` 组合多个处理器，同时实现日志记录、链路追踪、指标收集和录制。

### 类型化错误处理

`SynapticError` 为每个子系统提供一个变体（`Prompt`、`Model`、`Tool`、`Memory`、`Graph` 等）。这使得匹配特定的失败模式并提供针对性的恢复逻辑变得简单直接。

### 组合优于继承

Synaptic 倾向于组合而非深层 trait 层次结构。`CachedChatModel` 包装任意 `ChatModel`。`RetryChatModel` 包装任意 `ChatModel`。`RunnableWithFallbacks` 包装任意 `Runnable`。你通过包装来叠加行为，而非通过扩展基类。
