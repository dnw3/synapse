# Integrations

Synaptic provides optional integration crates that connect to external services. Each integration is gated behind a Cargo feature flag and adds no overhead when not enabled.

## Available Integrations

| Integration | Feature | Purpose |
|-------------|---------|---------|
| [OpenAI-Compatible Providers](openai-compatible.md) | `openai` | Groq, DeepSeek, Fireworks, Together, xAI, MistralAI, HuggingFace, Cohere, OpenRouter |
| [Azure OpenAI](azure-openai.md) | `openai` | Azure-hosted OpenAI models (chat + embeddings) |
| [AWS Bedrock](bedrock.md) | `bedrock` | AWS Bedrock foundation models (Claude, Llama, Mistral, etc.) |
| [Cohere Reranker](cohere.md) | `cohere` | Document reranking for improved retrieval quality |
| [Qdrant](qdrant.md) | `qdrant` | Vector store backed by the Qdrant vector database |
| [PgVector](pgvector.md) | `pgvector` | Vector store backed by PostgreSQL with the pgvector extension |
| [Pinecone](pinecone.md) | `pinecone` | Managed vector store backed by Pinecone |
| [Chroma](chroma.md) | `chroma` | Open-source vector store backed by Chroma |
| [MongoDB Atlas](mongodb.md) | `mongodb` | Vector search backed by MongoDB Atlas |
| [Elasticsearch](elasticsearch.md) | `elasticsearch` | Vector store backed by Elasticsearch kNN |
| [Redis](redis.md) | `redis` | Key-value store and LLM response cache backed by Redis |
| [SQLite Cache](sqlite.md) | `sqlite` | Persistent LLM response cache backed by SQLite |
| [PDF Loader](pdf.md) | `pdf` | Document loader for PDF files |
| [Tavily Search](tavily.md) | `tavily` | Web search tool for agents |

## Enabling integrations

Add the desired feature flags to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant", "redis"] }
```

You can combine any number of feature flags. Each integration pulls in only the dependencies it needs.

## Trait compatibility

Every integration implements a core Synaptic trait, so it plugs directly into the existing framework:

- **OpenAI-Compatible**, **Azure OpenAI**, and **Bedrock** implement `ChatModel` -- use them anywhere a model is accepted.
- **OpenAI-Compatible** (MistralAI, HuggingFace, Cohere) and **Azure OpenAI** also implement `Embeddings`.
- **Cohere Reranker** implements `DocumentCompressor` -- use it with `ContextualCompressionRetriever` for two-stage retrieval.
- **Qdrant**, **PgVector**, **Pinecone**, **Chroma**, **MongoDB Atlas**, and **Elasticsearch** implement `VectorStore` -- use them with `VectorStoreRetriever` or any component that accepts `&dyn VectorStore`.
- **Redis Store** implements `Store` -- use it anywhere `InMemoryStore` is used, including agent `ToolRuntime` injection.
- **Redis Cache** and **SQLite Cache** implement `LlmCache` -- wrap any `ChatModel` with `CachedChatModel` for persistent response caching.
- **PDF Loader** implements `Loader` -- use it in RAG pipelines alongside `TextSplitter`, `Embeddings`, and `VectorStore`.
- **Tavily Search** implements `Tool` -- register it with an agent for web search capabilities.

## Guides

### LLM Providers
- [OpenAI-Compatible Providers](openai-compatible.md) -- Groq, DeepSeek, Fireworks, Together, xAI, MistralAI, HuggingFace, Cohere, OpenRouter
- [Azure OpenAI](azure-openai.md) -- Azure-hosted OpenAI models
- [AWS Bedrock](bedrock.md) -- AWS Bedrock foundation models

### Reranking
- [Cohere Reranker](cohere.md) -- document reranking for improved retrieval

### Vector Stores
- [Qdrant Vector Store](qdrant.md) -- store and search embeddings with Qdrant
- [PgVector](pgvector.md) -- store and search embeddings with PostgreSQL + pgvector
- [Pinecone Vector Store](pinecone.md) -- managed vector store with Pinecone
- [Chroma Vector Store](chroma.md) -- open-source embedding database
- [MongoDB Atlas Vector Search](mongodb.md) -- vector search with MongoDB Atlas
- [Elasticsearch Vector Store](elasticsearch.md) -- vector search with Elasticsearch kNN

### Storage & Caching
- [Redis Store & Cache](redis.md) -- persistent key-value storage and LLM caching with Redis
- [SQLite Cache](sqlite.md) -- local LLM response caching with SQLite

### Loaders & Tools
- [PDF Loader](pdf.md) -- load documents from PDF files
- [Tavily Search Tool](tavily.md) -- web search tool for agents
