//! Synapse — A Rust agent framework with LangChain-compatible architecture.
//!
//! This crate re-exports all Synapse sub-crates for convenient single-import usage.
//! Enable features to control which modules are available.
//!
//! # Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `default` | `models`, `runnables`, `prompts`, `parsers`, `tools`, `callbacks` |
//! | `agent` | `default` + `graph`, `memory` |
//! | `rag` | `default` + `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores` |
//! | `full` | All features enabled |
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use synaptic::core::{ChatModel, Message, ChatRequest, ToolChoice};
//! use synaptic::models::OpenAiChatModel;
//! use synaptic::runnables::{Runnable, RunnableLambda, RunnableAssign, RunnablePick};
//! ```

/// Core traits and types: ChatModel, Message, ToolChoice, SynapseError, RunnableConfig, etc.
/// Always available.
pub use synaptic_core as core;

/// Chat model adapters: OpenAI, Anthropic, Gemini, Ollama, plus test doubles and wrappers.
#[cfg(feature = "models")]
pub use synaptic_models as models;

/// LCEL composition: Runnable trait (with stream), BoxRunnable (with bind), pipe operator,
/// Lambda, Parallel, Branch, Assign, Pick, Fallbacks, etc.
#[cfg(feature = "runnables")]
pub use synaptic_runnables as runnables;

/// Prompt templates: ChatPromptTemplate, FewShotChatMessagePromptTemplate.
#[cfg(feature = "prompts")]
pub use synaptic_prompts as prompts;

/// Output parsers: Str, Json, Structured, List, Enum.
#[cfg(feature = "parsers")]
pub use synaptic_parsers as parsers;

/// Tool registry and execution.
#[cfg(feature = "tools")]
pub use synaptic_tools as tools;

/// Memory strategies: Buffer, Window, Summary, SummaryBuffer, TokenBuffer, RunnableWithMessageHistory.
#[cfg(feature = "memory")]
pub use synaptic_memory as memory;

/// Callback handlers: Recording, Tracing, Composite.
#[cfg(feature = "callbacks")]
pub use synaptic_callbacks as callbacks;

/// Retrieval: Retriever trait, BM25, MultiQuery, Ensemble, Compression, SelfQuery, ParentDocument, Document.
#[cfg(feature = "retrieval")]
pub use synaptic_retrieval as retrieval;

/// Document loaders: Text, JSON, CSV, Directory.
#[cfg(feature = "loaders")]
pub use synaptic_loaders as loaders;

/// Text splitters: Character, Recursive, Markdown, Token.
#[cfg(feature = "splitters")]
pub use synaptic_splitters as splitters;

/// Embeddings: trait, Fake, OpenAI, Ollama.
#[cfg(feature = "embeddings")]
pub use synaptic_embeddings as embeddings;

/// Vector stores: InMemory, VectorStoreRetriever.
#[cfg(feature = "vectorstores")]
pub use synaptic_vectorstores as vectorstores;

/// Graph agent orchestration: StateGraph, CompiledGraph (with stream), GraphEvent, StreamMode, checkpointing.
#[cfg(feature = "graph")]
pub use synaptic_graph as graph;

/// LLM caching: InMemory, Semantic, CachedChatModel.
#[cfg(feature = "cache")]
pub use synaptic_cache as cache;

/// Evaluation: Evaluator trait, evaluators, Dataset.
#[cfg(feature = "eval")]
pub use synaptic_eval as eval;
