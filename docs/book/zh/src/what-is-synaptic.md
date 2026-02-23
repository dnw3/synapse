# 什么是 Synaptic？

Synaptic 是一个用于构建 AI Agent、链式调用和检索管道的 Rust 框架。它遵循与 [LangChain](https://python.langchain.com/)（Python）相同的架构和抽象，并将其转化为地道的 Rust 风格：强类型、异步原生和零开销抽象。

如果你使用过 LangChain Python，那么你已经熟悉了 Synaptic 的思维模型。Synaptic 提供相同的可组合构建块 -- Chat Models、Prompts、Output Parsers、Runnables、Tools、Memory、Graph 和 Retrieval -- 但在编译时而非运行时捕获错误。

## LangChain 到 Synaptic 的映射

下表展示了 LangChain Python 的核心概念如何映射到 Synaptic Rust 的对应实现：

| LangChain (Python) | Synaptic (Rust) | 所在 Crate |
|---|---|---|
| `ChatOpenAI` | `OpenAiChatModel` | `synaptic-openai` |
| `ChatAnthropic` | `AnthropicChatModel` | `synaptic-anthropic` |
| `ChatGoogleGenerativeAI` | `GeminiChatModel` | `synaptic-gemini` |
| `HumanMessage` / `AIMessage` | `Message::human()` / `Message::ai()` | `synaptic-core` |
| `RunnableSequence` / LCEL `\|` | `BoxRunnable` / `\|` 管道运算符 | `synaptic-runnables` |
| `RunnableLambda` | `RunnableLambda` | `synaptic-runnables` |
| `RunnableParallel` | `RunnableParallel` | `synaptic-runnables` |
| `RunnableBranch` | `RunnableBranch` | `synaptic-runnables` |
| `RunnablePassthrough.assign()` | `RunnableAssign` | `synaptic-runnables` |
| `ChatPromptTemplate` | `ChatPromptTemplate` | `synaptic-prompts` |
| `ToolNode` | `ToolNode` | `synaptic-graph` |
| `StateGraph` | `StateGraph` | `synaptic-graph` |
| `create_react_agent` | `create_react_agent` | `synaptic-graph` |
| `InMemorySaver` | `StoreCheckpointer` | `synaptic-graph` |
| `StrOutputParser` | `StrOutputParser` | `synaptic-parsers` |
| `JsonOutputParser` | `JsonOutputParser` | `synaptic-parsers` |
| `VectorStoreRetriever` | `VectorStoreRetriever` | `synaptic-vectorstores` |
| `RecursiveCharacterTextSplitter` | `RecursiveCharacterTextSplitter` | `synaptic-splitters` |
| `OpenAIEmbeddings` | `OpenAiEmbeddings` | `synaptic-openai` |

## 与 LangChain Python 的关键差异

虽然架构兼容，但 Synaptic 做出了符合 Rust 惯例的设计选择：

- **Message 是一个标签枚举（tagged enum）**，而非类继承体系。你通过工厂方法构建消息，如 `Message::human("hello")`，而不是实例化类。
- **ChatRequest 使用构造函数**加链式方法：`ChatRequest::new(messages).with_tools(tools).with_tool_choice(ToolChoice::Auto)`。
- **所有 trait 都是异步的**，通过 `#[async_trait]` 实现。`chat()`、`invoke()` 和 `call()` 都是异步函数。
- **并发使用基于 `Arc` 的共享**。注册表使用 `Arc<RwLock<_>>`，回调和 Memory 使用 `Arc<tokio::sync::Mutex<_>>`。
- **错误是有类型的**。`SynapticError` 是一个包含 19 个变体的枚举（每个子系统一个），而非通用异常。
- **流式处理基于 trait**。`ChatModel::stream_chat()` 返回 `ChatStream`（`AIMessageChunk` 的 pinned `Stream`），Graph 流式处理则产出 `GraphEvent` 值。

## 何时使用 Synaptic

Synaptic 适用于以下场景：

- **性能关键的 AI 应用** -- Rust 的零开销抽象和无垃圾回收机制使 Synaptic 适合高吞吐、低延迟的 Agent 工作负载。没有 Python GIL 限制并发。
- **Rust 生态系统集成** -- 如果你的应用已经用 Rust 编写（Axum/Actix Web 服务、CLI 工具、嵌入式系统），Synaptic 让你无需跨越 FFI 边界或管理 Python 子进程即可添加 AI Agent 能力。
- **编译时安全** -- 工具参数 schema、消息类型和 Runnable 管道签名都由编译器检查。重构工具的输入类型时，编译器会在每个调用点报错，而不是在生产环境中运行时崩溃。
- **可部署的二进制文件** -- Synaptic 编译为单个静态二进制文件，无运行时依赖。不需要 Python 解释器、虚拟环境或 pip install。
- **并发 Agent 工作负载** -- Tokio 的异步运行时让你在单台机器上高效调度数百个并发 Agent 会话。

## 何时不使用 Synaptic

- 如果你的团队主要使用 Python 编写代码，且快速原型开发的速度比运行时性能更重要，LangChain Python 是更务实的选择。
- 如果你需要使用完整的 LangChain 第三方集成生态（数百种向量存储、文档加载器和模型提供商），LangChain Python 目前拥有更广泛的覆盖。
