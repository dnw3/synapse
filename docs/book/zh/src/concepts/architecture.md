# 架构

Synaptic 采用分层的 Cargo 工作区架构，由多个库 crate、1 个门面 crate 和若干示例二进制程序组成。每一层都在下层的基础上构建，职责清晰、边界明确。

## Crate 分层结构

```text
+------------------------------------------+
|           应用层（你的代码）                 |
+------------------------------------------+
|       synaptic（门面: 重新导出所有）          |
+------------------------------------------+
|  组合层: graph, runnables, eval           |
+------------------------------------------+
|  提供商: openai, anthropic, gemini, ollama |
+------------------------------------------+
|  实现层: models, memory, callbacks,       |
|         prompts, parsers, tools, cache    |
+------------------------------------------+
|  集成层: qdrant, pgvector, redis, pdf     |
+------------------------------------------+
|  检索管道: loaders, splitters, embeddings,|
|           vectorstores, retrieval         |
+------------------------------------------+
|         synaptic-core（核心 trait 和类型）   |
+------------------------------------------+
```

### 核心层

**`synaptic-core`** 定义了所有共享的 trait 和类型，是其他所有 crate 的基础依赖。它只包含 trait 定义和类型，不包含任何具体实现。这意味着：

- 任何 crate 都可以依赖 `synaptic-core` 而不引入额外的编译依赖
- 你可以编写自己的 `ChatModel` 实现而不依赖 `synaptic-models`
- 测试可以使用简单的 mock 而不需要真实的 HTTP 客户端

核心内容包括：

- **Trait**: `ChatModel`、`Tool`、`MemoryStore`、`CallbackHandler`
- **类型**: `Message`、`ChatRequest`、`ChatResponse`、`ToolCall`、`ToolDefinition`、`ToolChoice`、`AIMessageChunk`、`TokenUsage`、`RunEvent`、`RunnableConfig`
- **错误类型**: `SynapticError`（19 个变体，覆盖所有子系统）
- **流类型**: `ChatStream`（`Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send>>`）

### 实现层

每个 crate 实现一个核心 trait 或提供一个聚焦的功能领域：

| Crate | 职责 |
|---|---|
| `synaptic-models` | `ProviderBackend` 抽象、`ScriptedChatModel` 测试替身、ChatModel 包装器（重试、速率限制、缓存、结构化输出、BoundTools） |
| `synaptic-tools` | `ToolRegistry` 工具注册表和 `SerialToolExecutor` 串行执行器 |
| `synaptic-memory` | 记忆策略：Buffer、Window、Summary、Token Buffer、Summary Buffer，以及 `RunnableWithMessageHistory` |
| `synaptic-callbacks` | `RecordingCallback`（录制回调）、`TracingCallback`（链路追踪回调）、`CompositeCallback`（组合回调） |
| `synaptic-prompts` | `PromptTemplate`（模板插值）、`ChatPromptTemplate`（聊天提示模板）、`FewShotChatMessagePromptTemplate`（少样本提示模板） |
| `synaptic-parsers` | 输出解析器：字符串、JSON、结构化、列表、枚举、布尔、XML、Markdown 列表、编号列表 |
| `synaptic-cache` | `InMemoryCache`（内存缓存）、`SemanticCache`（语义缓存）、`CachedChatModel`（缓存模型包装器） |

### 提供商层

每个 LLM 提供商由独立的 crate 提供：

| Crate | 职责 |
|---|---|
| `synaptic-openai` | `OpenAiChatModel`、`OpenAiEmbeddings` |
| `synaptic-anthropic` | `AnthropicChatModel` |
| `synaptic-gemini` | `GeminiChatModel` |
| `synaptic-ollama` | `OllamaChatModel`、`OllamaEmbeddings` |

### 组合层

这些 crate 提供更高层级的编排能力：

| Crate | 职责 |
|---|---|
| `synaptic-runnables` | `Runnable` trait，支持 `invoke()`/`batch()`/`stream()`；`BoxRunnable` 支持管道运算符 `\|` 进行 LCEL 风格的链式组合；`RunnableLambda`、`RunnableParallel`、`RunnableBranch`、`RunnableAssign`、`RunnablePick`、`RunnableWithFallbacks` 等组合类型 |
| `synaptic-graph` | LangGraph 风格的状态机：`StateGraph` 构建器、`CompiledGraph` 可执行图、`ToolNode` 工具节点、`create_react_agent` 预构建 Agent、`Checkpointer` + `StoreCheckpointer` 检查点持久化，支持图流式执行和可视化 |

### 检索管道

以下 crate 构成了文档摄取和检索的完整管道：

| Crate | 职责 |
|---|---|
| `synaptic-loaders` | 文档加载器：`TextLoader`、`JsonLoader`、`CsvLoader`、`DirectoryLoader`、`FileLoader`、`MarkdownLoader`、`WebBaseLoader` |
| `synaptic-splitters` | 文本分割器：`CharacterTextSplitter`、`RecursiveCharacterTextSplitter`、`MarkdownHeaderTextSplitter`、`TokenTextSplitter` |
| `synaptic-embeddings` | 嵌入模型：`Embeddings` trait、`FakeEmbeddings`、`CacheBackedEmbeddings`（提供商嵌入模型已拆分到 `synaptic-openai` 和 `synaptic-ollama`） |
| `synaptic-vectorstores` | 向量存储：`VectorStore` trait，`InMemoryVectorStore`、`VectorStoreRetriever`、`MultiVectorRetriever` |
| `synaptic-retrieval` | 检索器：`Retriever` trait，`BM25Retriever`、`MultiQueryRetriever`、`EnsembleRetriever`、`ContextualCompressionRetriever`、`SelfQueryRetriever`、`ParentDocumentRetriever` |

数据流方向为：

```text
loaders --> splitters --> embeddings --> vectorstores --> retrieval
```

### 评估

| Crate | 职责 |
|---|---|
| `synaptic-eval` | `Evaluator` trait，提供 `ExactMatchEvaluator`、`RegexMatchEvaluator`、`JsonValidityEvaluator`、`EmbeddingDistanceEvaluator`、`LLMJudgeEvaluator` 评估器，以及 `Dataset` 和批量评估管道 |

### 集成层

这些 crate 提供外部系统的集成：

| Crate | 职责 |
|---|---|
| `synaptic-qdrant` | Qdrant 向量存储（`QdrantVectorStore`） |
| `synaptic-pgvector` | PostgreSQL pgvector 向量存储（`PgVectorStore`） |
| `synaptic-redis` | Redis 存储和缓存（`RedisStore`、`RedisCache`） |
| `synaptic-pdf` | PDF 文档加载器（`PdfLoader`） |

### 门面

**`synaptic`** 重新导出所有子 crate，提供便捷的单入口导入方式：

```rust
use synaptic::core::{ChatModel, Message, ChatRequest};
use synaptic::openai::OpenAiChatModel;
use synaptic::runnables::{Runnable, RunnableLambda};
use synaptic::graph::{StateGraph, create_react_agent};
```

## 依赖关系图

所有 crate 都依赖 `synaptic-core` 获取共享的 trait 和类型。高层 crate 依赖其下层：

```text
                         +----------+
                         | synaptic |  (门面: 重新导出所有)
                         +----+-----+
                              |
       +----------------------+----------------------+
       |                      |                      |
  +----+-----+          +----+-----+          +-----+----+
  |  graph   |          |runnables |          |   eval   |
  +----+-----+          +----+-----+          +-----+----+
       |                     |                      |
  +----+--------+------------+----------+-----------+
  |    |        |            |          |           |
+--+--++---++---+-++-------++---+--++--+---++------+--+
|mod- ||too-||mem- ||promp-||pars-||cache ||callba-|
|els  ||ls  ||ory  ||ts   ||ers  ||     ||cks    |
+--+--++--+-++--+--++---+--++--+--++--+--++---+---+
   |      |      |       |       |       |       |
   +------+------+-------+-------+-------+-------+
   |              synaptic-core                    |
   |  (ChatModel, Tool, Message, SynapticError, .)|
   +----------------------------------------------+
```

## 设计原则

### 异步优先，基于 `#[async_trait]`

Synaptic 中的所有 trait 都是异步的。`ChatModel::chat()`、`Tool::call()`、`MemoryStore::load()` 和 `Runnable::invoke()` 全部是异步方法。这意味着你可以在任何实现中自由地 `await` 网络调用、数据库查询和并发操作，而不会阻塞运行时。框架使用 Tokio 作为异步运行时。

### 基于 `Arc` 的共享

Synaptic 对注册表（如 `ToolRegistry`）使用 `Arc<RwLock<_>>`，允许多个读者并发访问；对有状态组件（如回调和内存存储）使用 `Arc<tokio::sync::Mutex<_>>`，确保写入操作串行化。这样可以在异步任务和 Agent 会话之间安全共享数据。

### 基于 `session_id` 的会话隔离

内存存储和 Agent 运行通过 `session_id` 进行键控。多个对话可以在同一个模型和工具集上并发运行，而不会出现状态泄漏。这在 Web 应用中尤其重要——你可以用用户 ID、对话线程 ID 或两者的组合作为 session_id。

### 事件驱动回调

`CallbackHandler` trait 在每个生命周期阶段接收 `RunEvent` 值（运行开始、LLM 调用、工具调用、运行完成、运行失败）。你可以使用 `CompositeCallback` 组合多个处理器，同时实现日志记录、链路追踪、指标采集和录制。

### 类型化错误处理

`SynapticError` 为每个子系统定义了独立的变体（`Prompt`、`Model`、`Tool`、`Memory`、`Graph` 等）。这使得针对特定故障模式进行匹配和提供定向恢复逻辑变得简单直观。

### 组合优于继承

Synaptic 倾向于组合而非深层 trait 继承。`CachedChatModel` 包装任意 `ChatModel`，`RetryChatModel` 包装任意 `ChatModel`，`RunnableWithFallbacks` 包装任意 `Runnable`。你通过包装来叠加行为，而不是通过继承基类来扩展功能。

## Provider 适配

所有 LLM 提供商适配器通过 `ProviderBackend` trait 抽象 HTTP 层：

```text
ChatModel trait
    +-- OpenAiChatModel
            +-- ProviderBackend trait
                    |-- HttpBackend（生产环境，使用 reqwest）
                    +-- FakeBackend（测试环境，返回预设响应）
```

这种设计使得：
- 模型适配器的逻辑可以独立于 HTTP 客户端进行测试
- 替换 HTTP 层不需要修改模型逻辑
- 测试时无需启动真实 HTTP 服务
