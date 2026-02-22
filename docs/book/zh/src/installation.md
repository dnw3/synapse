# 安装

## 前置要求

- **Rust 1.88 或更高版本** -- Synaptic 的最低支持 Rust 版本（MSRV）为 1.88。使用 `rustup update` 更新你的工具链。
- **Cargo** -- Rust 的包管理器，随 Rust 一起安装。

## 添加依赖

### 使用 facade crate（推荐）

`synaptic` facade crate 重新导出所有子 crate。使用 **feature flags** 控制编译哪些模块。

### Feature Flags

Synaptic 提供类似 tokio 的细粒度 feature flags：

```toml
[dependencies]
# 全量引入（等同之前的默认行为）
synaptic = { version = "0.2", features = ["full"] }

# Agent 开发（自动包含 openai, tools, graph, memory 等）
synaptic = { version = "0.2", features = ["agent"] }

# RAG 应用（自动包含 openai, retrieval, loaders, splitters, embeddings, vectorstores 等）
synaptic = { version = "0.2", features = ["rag"] }

# Agent + RAG
synaptic = { version = "0.2", features = ["agent", "rag"] }

# 最小化 — 只要 OpenAI 模型调用
synaptic = { version = "0.2", features = ["openai"] }

# 使用 Anthropic
synaptic = { version = "0.2", features = ["anthropic"] }

# 全部四个提供商
synaptic = { version = "0.2", features = ["models"] }

# 精细控制
synaptic = { version = "0.2", features = ["openai", "graph", "cache"] }
```

| Feature | 说明 |
|---------|------|
| **`default`** | `runnables`, `prompts`, `parsers`, `tools`, `callbacks`（不包含任何提供商） |
| **`agent`** | `default` + `openai`, `graph`, `memory` |
| **`rag`** | `default` + `openai`, `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores` |
| **`full`** | 启用全部 features |

**提供商 features：**

| Feature | 说明 |
|---------|------|
| `openai` | OpenAI 提供商（`OpenAiChatModel`、`OpenAiEmbeddings`） |
| `anthropic` | Anthropic 提供商（`AnthropicChatModel`） |
| `gemini` | Google Gemini 提供商（`GeminiChatModel`） |
| `ollama` | Ollama 提供商（`OllamaChatModel`、`OllamaEmbeddings`） |
| `models` | 全部六个提供商（openai + anthropic + gemini + ollama + bedrock + cohere） |
| `model-utils` | `ProviderBackend` 抽象 + 包装器（`ScriptedChatModel`、`RetryChatModel` 等） |

**集成 features：**

| Feature | 说明 |
|---------|------|
| `qdrant` | Qdrant 向量存储（`QdrantVectorStore`） |
| `pgvector` | PostgreSQL pgvector 向量存储（`PgVectorStore`） |
| `redis` | Redis 存储和缓存（`RedisStore`、`RedisCache`） |
| `pdf` | PDF 文档加载器（`PdfLoader`） |
| `bedrock` | AWS Bedrock ChatModel（`synaptic-bedrock`） |
| `cohere` | Cohere Reranker（`synaptic-cohere`） |
| `pinecone` | Pinecone 向量存储（`synaptic-pinecone`） |
| `chroma` | Chroma 向量存储（`synaptic-chroma`） |
| `mongodb` | MongoDB Atlas 向量搜索（`synaptic-mongodb`） |
| `elasticsearch` | Elasticsearch 向量存储（`synaptic-elasticsearch`） |
| `sqlite` | SQLite LLM 缓存（`synaptic-sqlite`） |
| `tavily` | Tavily 搜索工具（`synaptic-tavily`） |

单独可用的 features：`openai`, `anthropic`, `gemini`, `ollama`, `bedrock`, `cohere`, `models`, `model-utils`, `runnables`, `prompts`, `parsers`, `tools`, `memory`, `callbacks`, `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores`, `graph`, `cache`, `eval`, `store`, `middleware`, `mcp`, `macros`, `deep`, `qdrant`, `pgvector`, `redis`, `pdf`, `pinecone`, `chroma`, `mongodb`, `elasticsearch`, `sqlite`, `tavily`。

**高级 features：**

| Feature | 说明 |
|---------|------|
| `store` | 支持命名空间层次和可选语义搜索的键值存储 |
| `middleware` | Agent 中间件链（工具调用限制、人机协作、摘要、上下文编辑） |
| `mcp` | Model Context Protocol 客户端（Stdio/SSE/HTTP 传输） |
| `macros` | 过程宏（`#[tool]`、`#[chain]`、`#[entrypoint]`、`#[traceable]`） |
| `deep` | Deep Agent 框架（Backend、文件系统工具、子 Agent、技能） |

`core` 模块（核心 trait 和类型）始终可用，不受 feature 选择影响。

然后在代码中使用：

```rust
use synaptic::core::{ChatModel, Message, ChatRequest};
use synaptic::openai::OpenAiChatModel;
```

### 按需引入单个 crate

如果你只需要特定功能，也可以单独添加所需的 crate：

```toml
[dependencies]
synaptic-core = "0.2"
synaptic-models = "0.2"
synaptic-runnables = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### 常用依赖组合

以下是几种常见场景的依赖配置：

**基础 LLM 调用（OpenAI）：**

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**带工具调用的 Agent：**

```toml
[dependencies]
synaptic = { version = "0.2", features = ["agent"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
async-trait = "0.1"
```

**RAG 应用：**

```toml
[dependencies]
synaptic = { version = "0.2", features = ["rag"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 环境变量

根据你使用的 LLM 提供商，需要设置相应的 API 密钥环境变量：

| 提供商 | 环境变量 |
|---|---|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Google Gemini | `GOOGLE_API_KEY` |
| Ollama | 无需密钥（默认连接 `http://localhost:11434`） |

可以通过 `.env` 文件或直接在 shell 中设置：

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

> **注意：** 使用 `ScriptedChatModel`（测试替身，需启用 `model-utils` feature）时不需要任何 API 密钥，非常适合本地开发和测试。

## 验证安装

创建一个新项目并验证安装是否成功：

```bash
cargo new my-synaptic-app
cd my-synaptic-app
```

在 `Cargo.toml` 中添加依赖后，运行：

```bash
cargo build
```

如果编译成功，说明安装完成。接下来可以前往[快速开始](quickstart.md)编写你的第一个 Synaptic 程序。
