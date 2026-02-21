# What is Synaptic?

Synaptic is a Rust framework for building AI agents, chains, and retrieval pipelines. It follows the same architecture and abstractions as [LangChain](https://python.langchain.com/) (Python), translated into idiomatic Rust with strong typing, async-native design, and zero-cost abstractions.

If you have used LangChain in Python, you already know the mental model. Synaptic provides the same composable building blocks -- chat models, prompts, output parsers, runnables, tools, memory, graphs, and retrieval -- but catches errors at compile time instead of runtime.

## LangChain to Synaptic Mapping

The table below shows how core LangChain Python concepts map to their Synaptic Rust equivalents:

| LangChain (Python) | Synaptic (Rust) | Crate |
|---|---|---|
| `ChatOpenAI` | `OpenAiChatModel` | `synaptic-openai` |
| `ChatAnthropic` | `AnthropicChatModel` | `synaptic-anthropic` |
| `ChatGoogleGenerativeAI` | `GeminiChatModel` | `synaptic-gemini` |
| `HumanMessage` / `AIMessage` | `Message::human()` / `Message::ai()` | `synaptic-core` |
| `RunnableSequence` / LCEL `\|` | `BoxRunnable` / `\|` pipe operator | `synaptic-runnables` |
| `RunnableLambda` | `RunnableLambda` | `synaptic-runnables` |
| `RunnableParallel` | `RunnableParallel` | `synaptic-runnables` |
| `RunnableBranch` | `RunnableBranch` | `synaptic-runnables` |
| `RunnablePassthrough.assign()` | `RunnableAssign` | `synaptic-runnables` |
| `ChatPromptTemplate` | `ChatPromptTemplate` | `synaptic-prompts` |
| `ToolNode` | `ToolNode` | `synaptic-graph` |
| `StateGraph` | `StateGraph` | `synaptic-graph` |
| `create_react_agent` | `create_react_agent` | `synaptic-graph` |
| `InMemorySaver` | `MemorySaver` | `synaptic-graph` |
| `StrOutputParser` | `StrOutputParser` | `synaptic-parsers` |
| `JsonOutputParser` | `JsonOutputParser` | `synaptic-parsers` |
| `VectorStoreRetriever` | `VectorStoreRetriever` | `synaptic-vectorstores` |
| `RecursiveCharacterTextSplitter` | `RecursiveCharacterTextSplitter` | `synaptic-splitters` |
| `OpenAIEmbeddings` | `OpenAiEmbeddings` | `synaptic-openai` |

## Key Differences from LangChain Python

While the architecture is compatible, Synaptic makes deliberate Rust-idiomatic choices:

- **Message is a tagged enum**, not a class hierarchy. You construct messages with factory methods like `Message::human("hello")` rather than instantiating classes.
- **ChatRequest uses a constructor** with builder methods: `ChatRequest::new(messages).with_tools(tools).with_tool_choice(ToolChoice::Auto)`.
- **All traits are async** via `#[async_trait]`. Every `chat()`, `invoke()`, and `call()` is an async function.
- **Concurrency uses `Arc`-based sharing**. Registries use `Arc<RwLock<_>>`, callbacks and memory use `Arc<tokio::sync::Mutex<_>>`.
- **Errors are typed**. `SynapticError` is an enum with 19 variants (one per subsystem), not a generic exception.
- **Streaming is trait-based**. `ChatModel::stream_chat()` returns a `ChatStream` (a pinned `Stream` of `AIMessageChunk`), and graph streaming yields `GraphEvent` values.

## When to Use Synaptic

Synaptic is a good fit when you need:

- **Performance-critical AI applications** -- Rust's zero-cost abstractions and lack of garbage collection make Synaptic suitable for high-throughput, low-latency agent workloads. There is no Python GIL limiting concurrency.
- **Rust ecosystem integration** -- If your application is already written in Rust (web servers with Axum/Actix, CLI tools, embedded systems), Synaptic lets you add AI agent capabilities without crossing an FFI boundary or managing a Python subprocess.
- **Compile-time safety** -- Tool argument schemas, message types, and runnable pipeline signatures are all checked by the compiler. Refactoring a tool's input type produces compile errors at every call site, not runtime crashes in production.
- **Deployable binaries** -- Synaptic compiles to a single static binary with no runtime dependencies. No Python interpreter, no virtual environment, no pip install.
- **Concurrent agent workloads** -- Tokio's async runtime lets you run hundreds of concurrent agent sessions on a single machine with efficient task scheduling.

## When Not to Use Synaptic

- If your team primarily writes Python and rapid prototyping speed matters more than runtime performance, LangChain Python is the more pragmatic choice.
- If you need access to the full LangChain ecosystem of third-party integrations (hundreds of vector stores, document loaders, and model providers), LangChain Python has broader coverage today.
