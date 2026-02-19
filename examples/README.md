# Synaptic Examples

Runnable examples demonstrating Synaptic's features. Each example is a standalone Cargo binary.

## Running

```bash
cargo run -p <example_name>
```

Most examples use `ScriptedChatModel` or other test doubles and require no API keys. Examples that call real LLM providers need the corresponding environment variable set (e.g., `OPENAI_API_KEY`).

## Examples

| Example | Feature | Description |
|---------|---------|-------------|
| `tool_calling_basic` | Tools, Macros | Define tools with `#[tool]`, register them, and execute via `SerialToolExecutor` |
| `memory_chat` | Memory | Store and retrieve multi-turn conversations with session isolation |
| `memory_strategy` | Memory | Compare buffer, window, summary, and token-buffer memory strategies |
| `react_basic` | Graph, Tools | Build a ReAct agent that reasons and calls tools in a loop |
| `lcel_chain` | Runnables | Compose prompt templates, models, and parsers with the `\|` pipe operator |
| `streaming` | Streaming | Stream LLM responses token-by-token and through LCEL chains |
| `structured_output` | Models | Enforce JSON schema on model output with `StructuredOutputChatModel` |
| `prompt_parser_chain` | Prompts, Parsers | Chain a `ChatPromptTemplate` with a model and output parser |
| `rag_pipeline` | Retrieval | End-to-end RAG: load, split, embed, store, and retrieve documents |
| `caching` | Cache | Cache LLM responses with `InMemoryCache` and TTL expiry |
| `callbacks_tracing` | Callbacks | Structured tracing with `TracingCallback` and `RecordingCallback` |
| `evaluation` | Eval | Evaluate model outputs with exact match, regex, and LLM judge evaluators |
| `graph_visualization` | Graph | Render graph structure as Mermaid, ASCII, and DOT formats |
| `macros_showcase` | Macros | Demonstrate `#[tool]`, `#[chain]`, `#[entrypoint]`, and `#[traceable]` macros |
