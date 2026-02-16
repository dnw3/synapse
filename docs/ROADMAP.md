# Synapse Roadmap

## Implemented

- Phase 0: workspace setup and quality baseline
- Phase 1: core refactor — Message enum (System/Human/AI/Tool), AIMessageChunk streaming, RunnableConfig, SynapseError expansion (19 variants), Document metadata, ReAct pipeline with tool execution/memory/callbacks
- Phase 2: multi-provider model adapters (OpenAI, Anthropic, Gemini, Ollama) + streaming — ToolDefinition, ChatRequest.tools, ChatModel.stream_chat(), ProviderBackend abstraction (HttpBackend/FakeBackend), RetryChatModel, RateLimitedChatModel
- Phase 3: LCEL — Runnable composition protocol: Runnable<I,O> trait with invoke/batch/boxed, BoxRunnable with `|` pipe operator, RunnablePassthrough, RunnableLambda, RunnableSequence, RunnableParallel, RunnableBranch, RunnableWithFallbacks; updated SequentialChain
- Phase 4: prompt templates + output parsers — ChatPromptTemplate (MessageTemplate: System/Human/AI/Placeholder), FewShotChatMessagePromptTemplate, StrOutputParser, JsonOutputParser, StructuredOutputParser<T>, ListOutputParser, EnumOutputParser; all implement Runnable
- Phase 5: document pipeline — Loader async trait, TextLoader, JsonLoader, CsvLoader, DirectoryLoader; TextSplitter trait, CharacterTextSplitter, RecursiveCharacterTextSplitter, MarkdownHeaderTextSplitter
- Phase 6: embeddings + vector stores — Embeddings trait, FakeEmbeddings, OpenAiEmbeddings, OllamaEmbeddings; VectorStore trait, InMemoryVectorStore (cosine similarity), VectorStoreRetriever bridge to Retriever
- Foundations: runnable, chain, retrieval, loader, guardrail, eval baseline abstractions

## Next
- Phase 7: advanced retrieval (MultiQuery, SelfQuery, Ensemble, Compression, BM25)
- Phase 8: graph agent orchestration (StateGraph, checkpointing, human-in-the-loop)
- Phase 9: memory strategies (Buffer, Window, Summary, Token) + persistence backends
- Phase 10: caching, rate limiting, reliability
- Phase 11: observability (tracing, OpenTelemetry) + evaluation (LLM-as-judge, datasets)
- Phase 12: full LangChain parity + ecosystem (API server, CLI, unified facade crate)

See `docs/plans/2026-02-16-synapse-full-langchain-parity-design.md` for full design.
