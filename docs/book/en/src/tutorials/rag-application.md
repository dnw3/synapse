# Build a RAG Application

This tutorial walks you through building a Retrieval-Augmented Generation (RAG) pipeline with Synaptic. RAG is a pattern where you retrieve relevant documents from a knowledge base and include them as context in a prompt, so the LLM can answer questions grounded in your data rather than relying solely on its training.

## Prerequisites

Add the required Synaptic crates to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["rag"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## How RAG Works

A RAG pipeline has two phases:

```text
 Indexing (offline)                     Querying (online)
 ==================                     ==================

 +-----------+                          +-----------+
 | Documents |                          |   Query   |
 +-----+-----+                          +-----+-----+
       |                                      |
       v                                      v
 +-----+------+                         +-----+------+
 |   Split    |                         |  Retrieve  | <--- Vector Store
 +-----+------+                         +-----+------+
       |                                      |
       v                                      v
 +-----+------+                         +-----+------+
 |   Embed    |                         |  Augment   | (inject context into prompt)
 +-----+------+                         +-----+------+
       |                                      |
       v                                      v
 +-----+------+                         +-----+------+
 |   Store    | ---> Vector Store       |  Generate  | (LLM produces answer)
 +------------+                         +------------+
```

1. **Indexing** -- Load documents, split them into chunks, embed each chunk, and store the vectors.
2. **Querying** -- Embed the user's question, find the most similar chunks, include them in a prompt, and ask the LLM.

## Step 1: Load Documents

Synaptic provides several document loaders. `TextLoader` wraps an in-memory string into a `Document`. For files on disk, use `FileLoader`.

```rust
use synaptic::loaders::{Loader, TextLoader};

let loader = TextLoader::new(
    "rust-intro",
    "Rust is a systems programming language focused on safety, speed, and concurrency. \
     It achieves memory safety without a garbage collector through its ownership system. \
     Rust's type system and borrow checker ensure that references are always valid. \
     The language has grown rapidly since its 1.0 release in 2015 and is widely used \
     for systems programming, web backends, embedded devices, and command-line tools.",
);

let docs = loader.load().await?;
// docs[0].id == "rust-intro"
// docs[0].content == the full text above
```

Each `Document` has three fields:

- **`id`** -- a unique identifier (a string you provide).
- **`content`** -- the text content.
- **`metadata`** -- a `HashMap<String, serde_json::Value>` for arbitrary key-value pairs.

For loading files from disk, use `FileLoader`:

```rust
use synaptic::loaders::{Loader, FileLoader};

let loader = FileLoader::new("data/rust-book.txt");
let docs = loader.load().await?;
// docs[0].id == "data/rust-book.txt"
// docs[0].metadata["source"] == "data/rust-book.txt"
```

Other loaders include `JsonLoader`, `CsvLoader`, and `DirectoryLoader` (for loading many files at once with glob filtering).

## Step 2: Split Documents into Chunks

Large documents need to be split into smaller chunks so that retrieval can return focused, relevant passages instead of entire files. `RecursiveCharacterTextSplitter` tries a hierarchy of separators (`\n\n`, `\n`, ` `, `""`) and keeps chunks within a size limit.

```rust
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};

let splitter = RecursiveCharacterTextSplitter::new(100)
    .with_chunk_overlap(20);

let chunks = splitter.split_documents(docs);
for chunk in &chunks {
    println!("[{}] {} chars: {}...", chunk.id, chunk.content.len(), &chunk.content[..40]);
}
```

The splitter produces new `Document` values with IDs like `rust-intro-chunk-0`, `rust-intro-chunk-1`, etc. Each chunk inherits the parent document's metadata and gains a `chunk_index` metadata field.

Key parameters:

- **`chunk_size`** -- the maximum character length of each chunk (passed to `new()`).
- **`chunk_overlap`** -- how many characters from the end of one chunk overlap with the start of the next (set with `.with_chunk_overlap()`). Overlap helps preserve context across chunk boundaries.

Other splitters are available for specialized content: `CharacterTextSplitter`, `MarkdownHeaderTextSplitter`, `HtmlHeaderTextSplitter`, and `TokenTextSplitter`.

## Step 3: Embed and Store

Embeddings convert text into numerical vectors so that similarity can be computed mathematically. `FakeEmbeddings` provides deterministic, hash-based vectors for testing -- no API key required.

```rust
use std::sync::Arc;
use synaptic::embeddings::FakeEmbeddings;
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore};

let embeddings = Arc::new(FakeEmbeddings::new(128));

// Create a vector store and add the chunks
let store = InMemoryVectorStore::new();
let ids = store.add_documents(chunks, embeddings.as_ref()).await?;
println!("Indexed {} chunks", ids.len());
```

`InMemoryVectorStore` stores document vectors in memory and uses cosine similarity for search. For convenience, you can also create a pre-populated store in one step:

```rust
let store = InMemoryVectorStore::from_documents(chunks, embeddings.as_ref()).await?;
```

For production use, replace `FakeEmbeddings` with `OpenAiEmbeddings` (from `synaptic::openai`) or `OllamaEmbeddings` (from `synaptic::ollama`), which call real embedding APIs.

## Step 4: Retrieve Relevant Documents

Now you can search the vector store for chunks that are similar to a query:

```rust
use synaptic::vectorstores::VectorStore;

let results = store.similarity_search("What is Rust?", 3, embeddings.as_ref()).await?;
for doc in &results {
    println!("Found: {}", doc.content);
}
```

The second argument (`3`) is `k` -- the number of results to return.

### Using a Retriever

For a cleaner API that decouples retrieval logic from the store implementation, wrap the store in a `VectorStoreRetriever`:

```rust
use synaptic::retrieval::Retriever;
use synaptic::vectorstores::VectorStoreRetriever;

let retriever = VectorStoreRetriever::new(
    Arc::new(store),
    embeddings.clone(),
    3, // default k
);

let results = retriever.retrieve("What is Rust?", 3).await?;
```

The `Retriever` trait has a single method -- `retrieve(query, top_k)` -- and is implemented by many retrieval strategies in Synaptic:

- **`VectorStoreRetriever`** -- wraps any `VectorStore` for similarity search.
- **`BM25Retriever`** -- keyword-based scoring (no embeddings needed).
- **`MultiQueryRetriever`** -- generates multiple query variants with an LLM to improve recall.
- **`EnsembleRetriever`** -- combines multiple retrievers with Reciprocal Rank Fusion.

## Step 5: Generate an Answer

The final step combines retrieved context with the user's question in a prompt. Here is the complete pipeline:

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::ScriptedChatModel;
use synaptic::loaders::{Loader, TextLoader};
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore, VectorStoreRetriever};
use synaptic::retrieval::Retriever;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // 1. Load
    let loader = TextLoader::new(
        "rust-guide",
        "Rust is a systems programming language focused on safety, speed, and concurrency. \
         It achieves memory safety without a garbage collector through its ownership system. \
         Rust was first released in 2015 and has grown into one of the most loved languages \
         according to developer surveys.",
    );
    let docs = loader.load().await?;

    // 2. Split
    let splitter = RecursiveCharacterTextSplitter::new(100).with_chunk_overlap(20);
    let chunks = splitter.split_documents(docs);

    // 3. Embed and store
    let embeddings = Arc::new(FakeEmbeddings::new(128));
    let store = InMemoryVectorStore::from_documents(chunks, embeddings.as_ref()).await?;

    // 4. Retrieve
    let retriever = VectorStoreRetriever::new(Arc::new(store), embeddings.clone(), 2);
    let question = "When was Rust first released?";
    let relevant = retriever.retrieve(question, 2).await?;

    // 5. Build the augmented prompt
    let context = relevant
        .iter()
        .map(|doc| doc.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        "Answer the question based only on the following context:\n\n\
         {context}\n\n\
         Question: {question}"
    );

    // 6. Generate (using ScriptedChatModel for offline testing)
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("Rust was first released in 2015."),
            usage: None,
        },
    ]);

    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Answer questions using only the provided context."),
        Message::human(prompt),
    ]);

    let response = model.chat(request).await?;
    println!("Answer: {}", response.message.content());
    // Output: Answer: Rust was first released in 2015.

    Ok(())
}
```

In production, you would replace `ScriptedChatModel` with a real provider like `OpenAiChatModel` (from `synaptic::openai`) or `AnthropicChatModel` (from `synaptic::anthropic`).

## Building RAG with LCEL Chains

For a more composable approach, you can integrate the retrieval step into an LCEL pipeline using `RunnableParallel`, `RunnableLambda`, and the pipe operator. This lets you express the RAG pattern as a single chain:

```text
                    +---> retriever ---> format context ---+
                    |                                      |
  input (query) ---+                                      +---> prompt ---> model ---> parser
                    |                                      |
                    +---> passthrough (question) ----------+
```

Each step is a `Runnable`, and they compose with `|`. See the [Runnables how-to guides](../how-to/runnables/index.md) for details on `RunnableParallel` and `RunnableLambda`.

## Summary

In this tutorial you learned how to:

- Load documents with `TextLoader` and `FileLoader`
- Split documents into retrieval-friendly chunks with `RecursiveCharacterTextSplitter`
- Embed and store chunks in an `InMemoryVectorStore`
- Retrieve relevant documents with `VectorStoreRetriever`
- Combine retrieved context with a prompt to generate grounded answers

## Next Steps

- [Build a Graph Workflow](graph-workflow.md) -- orchestrate multi-step agent logic with a state graph
- [Retrieval How-to Guides](../how-to/retrieval/index.md) -- BM25, multi-query, ensemble, and compression retrievers
- [Retrieval Concepts](../concepts/retrieval.md) -- deeper look at embedding and retrieval strategies
