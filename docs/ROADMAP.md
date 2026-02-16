# Synapse Roadmap

## Implemented

- Phase 0: workspace setup and quality baseline
- Phase 1: core refactor — Message enum (System/Human/AI/Tool), AIMessageChunk streaming, RunnableConfig, SynapseError expansion (19 variants), Document metadata, ReAct pipeline with tool execution/memory/callbacks
- Phase 2: multi-provider model adapters (OpenAI, Anthropic, Gemini, Ollama) + streaming — ToolDefinition, ChatRequest.tools, ChatModel.stream_chat(), ProviderBackend abstraction (HttpBackend/FakeBackend), RetryChatModel, RateLimitedChatModel
- Phase 3: LCEL — Runnable composition protocol: Runnable<I,O> trait with invoke/batch/boxed, BoxRunnable with `|` pipe operator, RunnablePassthrough, RunnableLambda, RunnableSequence, RunnableParallel, RunnableBranch, RunnableWithFallbacks; updated SequentialChain
- Phase 4: prompt templates + output parsers — ChatPromptTemplate (MessageTemplate: System/Human/AI/Placeholder), FewShotChatMessagePromptTemplate, StrOutputParser, JsonOutputParser, StructuredOutputParser<T>, ListOutputParser, EnumOutputParser; all implement Runnable
- Phase 5: document pipeline — Loader async trait, TextLoader, JsonLoader, CsvLoader, DirectoryLoader; TextSplitter trait, CharacterTextSplitter, RecursiveCharacterTextSplitter, MarkdownHeaderTextSplitter
- Phase 6: embeddings + vector stores — Embeddings trait, FakeEmbeddings, OpenAiEmbeddings, OllamaEmbeddings; VectorStore trait, InMemoryVectorStore (cosine similarity), VectorStoreRetriever bridge to Retriever
- Phase 7: advanced retrieval — BM25Retriever (Okapi BM25 scoring, tunable k1/b), MultiQueryRetriever (LLM query variants + dedup), EnsembleRetriever (Reciprocal Rank Fusion with weights), ContextualCompressionRetriever + DocumentCompressor trait + EmbeddingsFilter (cosine similarity threshold)
- Phase 8: graph agent orchestration — StateGraph<S> builder, CompiledGraph<S> execution engine, State trait (merge/reduce), MessageState, Node<S> trait + FnNode, Edge + ConditionalEdge + RouterFn, Checkpointer trait + MemorySaver, interrupt_before/interrupt_after (human-in-the-loop), update_state(), ToolNode, create_react_agent(model, tools)
- Foundations: runnable, chain, retrieval, loader, guardrail, eval baseline abstractions

## Next
- Phase 9: memory strategies (Buffer, Window, Summary, Token) + persistence backends
- Phase 10: caching, rate limiting, reliability
- Phase 11: observability (tracing, OpenTelemetry) + evaluation (LLM-as-judge, datasets)
- Phase 12: full LangChain parity + ecosystem (API server, CLI, unified facade crate)

See `docs/plans/2026-02-16-synapse-full-langchain-parity-design.md` for full design.
