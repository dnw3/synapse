# Retrieval

Retrieval-Augmented Generation (RAG) grounds LLM responses in external knowledge. Instead of relying solely on what the model learned during training, a RAG system retrieves relevant documents at query time and includes them in the prompt. This page explains the retrieval pipeline's architecture, the role of each component, and the retriever types Synaptic provides.

## The Pipeline

A RAG pipeline has five stages:

```
Load  -->  Split  -->  Embed  -->  Store  -->  Retrieve
```

1. **Load**: Read raw content from files, databases, or the web into `Document` structs.
2. **Split**: Break large documents into smaller, semantically coherent chunks.
3. **Embed**: Convert text chunks into numerical vectors that capture meaning.
4. **Store**: Index the vectors for efficient similarity search.
5. **Retrieve**: Given a query, find the most relevant chunks.

Each stage has a dedicated trait and multiple implementations. You can mix and match implementations at each stage depending on your data sources and requirements.

## Document

The `Document` struct is the universal unit of content:

```rust
pub struct Document {
    pub id: Option<String>,
    pub content: String,
    pub metadata: HashMap<String, Value>,
}
```

- `content` holds the text.
- `metadata` holds arbitrary key-value pairs (source filename, page number, section heading, creation date, etc.).
- `id` is an optional unique identifier used by stores for upsert and delete operations.

Documents flow through every stage of the pipeline. Loaders produce them, splitters transform them (preserving and augmenting metadata), and retrievers return them.

## Loading

The `Loader` trait is async and returns a stream of documents:

| Loader | Source | Behavior |
|--------|--------|----------|
| `TextLoader` | Plain text files | One document per file |
| `JsonLoader` | JSON files | Configurable `id_key` and `content_key` extraction |
| `CsvLoader` | CSV files | Column-based, with metadata from other columns |
| `DirectoryLoader` | Directory of files | Recursive, with glob filtering to select file types |
| `FileLoader` | Single file | Generic file loading with configurable parser |
| `MarkdownLoader` | Markdown files | Markdown-aware parsing |
| `WebLoader` | URLs | Fetches and processes web content |

Loaders handle the mechanics of reading and parsing. They produce `Document` values with appropriate metadata (e.g., a `source` field with the file path).

## Splitting

Large documents must be split into chunks that fit within embedding models' context windows and that contain focused, coherent content. The `TextSplitter` trait provides:

```rust
pub trait TextSplitter: Send + Sync {
    fn split_text(&self, text: &str) -> Result<Vec<String>, SynapticError>;
    fn split_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>, SynapticError>;
}
```

| Splitter | Strategy |
|----------|----------|
| `CharacterTextSplitter` | Splits on a single separator (default: `"\n\n"`) with configurable chunk size and overlap |
| `RecursiveCharacterTextSplitter` | Tries a hierarchy of separators (`"\n\n"`, `"\n"`, `" "`, `""`) -- splits on the largest unit that fits within the chunk size |
| `MarkdownHeaderTextSplitter` | Splits on Markdown headers, adding header hierarchy to metadata |
| `HtmlHeaderTextSplitter` | Splits on HTML header tags, adding header hierarchy to metadata |
| `TokenTextSplitter` | Splits based on approximate token count (~4 chars/token heuristic, word-boundary aware) |
| `LanguageTextSplitter` | Splits code using language-aware separators (functions, classes, etc.) |

The most commonly used splitter is `RecursiveCharacterTextSplitter`. It produces chunks that respect natural document boundaries (paragraphs, then sentences, then words) and includes configurable overlap between chunks so that information at chunk boundaries is not lost.

`split_documents()` preserves the original document's metadata on each chunk, so you can trace every chunk back to its source.

## Embedding

Embedding models convert text into dense numerical vectors. Texts with similar meaning produce vectors that are close together in the vector space. The trait:

```rust
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, SynapticError>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError>;
}
```

Two methods because some providers optimize differently for documents (which may be batched) versus queries (single text, possibly with different prompt prefixes).

| Implementation | Description |
|----------------|-------------|
| `OpenAiEmbeddings` | OpenAI's embedding API (text-embedding-ada-002, etc.) |
| `OllamaEmbeddings` | Local Ollama embedding models |
| `FakeEmbeddings` | Deterministic vectors for testing (no API calls) |
| `CachedEmbeddings` | Wraps any `Embeddings` with a cache to avoid redundant API calls |

## Vector Storage

Vector stores hold embedded documents and support similarity search:

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add_documents(&self, docs: Vec<Document>, embeddings: Vec<Vec<f32>>) -> Result<Vec<String>, SynapticError>;
    async fn similarity_search(&self, query_embedding: &[f32], k: usize) -> Result<Vec<Document>, SynapticError>;
    async fn delete(&self, ids: &[String]) -> Result<(), SynapticError>;
}
```

`InMemoryVectorStore` uses cosine similarity with brute-force search. It stores documents and their embeddings in a `RwLock<HashMap>`, computes cosine similarity against all stored vectors at query time, and returns the top-k results. This is suitable for small to medium collections (thousands of documents). For larger collections, you would implement the `VectorStore` trait with a dedicated vector database.

## Retrieval

The `Retriever` trait is the query-time interface:

```rust
#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str) -> Result<Vec<Document>, SynapticError>;
}
```

A retriever takes a natural-language query and returns relevant documents. Synaptic provides seven retriever implementations, each with different strengths.

### InMemoryRetriever

The simplest retriever -- stores documents in memory and returns them based on keyword matching. Useful for testing and small collections.

### BM25Retriever

Implements the Okapi BM25 scoring algorithm, a classical information retrieval method that ranks documents by term frequency and inverse document frequency. No embeddings required -- purely lexical matching.

BM25 excels at exact keyword matching. If a user searches for "tokio runtime" and a document contains exactly those words, BM25 will rank it highly even if semantically similar documents that use different words score lower.

### MultiQueryRetriever

Uses an LLM to generate multiple query variants from the original query, then runs each variant through a base retriever and combines the results. This addresses the problem that a single query phrasing may miss relevant documents:

```
Original query: "How do I handle errors?"
Generated variants:
  - "What is the error handling approach?"
  - "How are errors propagated in the system?"
  - "What error types are available?"
```

### EnsembleRetriever

Combines results from multiple retrievers using Reciprocal Rank Fusion (RRF). A typical setup pairs BM25 (good at exact matches) with a vector store retriever (good at semantic matches):

The RRF algorithm assigns scores based on rank position across retrievers, so a document that appears in the top results of multiple retrievers gets a higher combined score.

### ContextualCompressionRetriever

Wraps a base retriever and compresses retrieved documents to remove irrelevant content. Uses a `DocumentCompressor` (such as `EmbeddingsFilter`, which filters out documents below a similarity threshold) to refine results after retrieval.

### SelfQueryRetriever

Uses an LLM to parse the user's query into a structured filter over document metadata, combined with a semantic search query. For example:

```
User query: "Find papers about transformers published after 2020"
Parsed:
  - Semantic query: "papers about transformers"
  - Metadata filter: year > 2020
```

This enables natural-language queries that combine semantic search with precise metadata filtering.

### ParentDocumentRetriever

Stores small child chunks for embedding (which improves retrieval precision) but returns the larger parent documents they came from (which provides more context to the LLM). This addresses the tension between small chunks (better for matching) and large chunks (better for context).

### MultiVectorRetriever

Similar to `ParentDocumentRetriever`, but implemented at the vector store level. `MultiVectorRetriever` stores child document embeddings in a `VectorStore` and maintains a separate docstore mapping child IDs to parent documents. At query time, it searches for matching child chunks and then looks up their parent documents for return. This is available in `synaptic-vectorstores`.

## Connecting Retrieval to Generation

Retrievers produce `Vec<Document>`. To use them in a RAG chain, you typically format the documents into a prompt and pass them to an LLM:

```rust
// Pseudocode for a RAG chain
let docs = retriever.retrieve("What is Synaptic?").await?;
let context = docs.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join("\n\n");
let prompt = format!("Context:\n{context}\n\nQuestion: What is Synaptic?");
```

Using LCEL, this can be composed into a reusable chain with `RunnableParallel` (to fetch context and pass through the question simultaneously), `RunnableLambda` (to format the prompt), and a chat model.

## See Also

- [Document Loaders](../how-to/retrieval/loaders.md) -- loading data from files and the web
- [Text Splitters](../how-to/retrieval/splitters.md) -- splitting documents into chunks
- [Embeddings](../how-to/retrieval/embeddings.md) -- embedding models for vector search
- [Vector Stores](../how-to/retrieval/vector-stores.md) -- storing and searching vectors
- [BM25 Retriever](../how-to/retrieval/bm25.md) -- keyword-based retrieval
- [Ensemble Retriever](../how-to/retrieval/ensemble.md) -- combining multiple retrievers
- [Self-Query Retriever](../how-to/retrieval/self-query.md) -- LLM-powered metadata filtering
- [Runnables & LCEL](runnables-lcel.md) -- composing retrieval into chains
