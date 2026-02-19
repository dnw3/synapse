# Runnables & LCEL

The LangChain Expression Language (LCEL) is a composition system for building data processing pipelines. In Synaptic, this is implemented through the `Runnable` trait and a set of combinators that let you pipe, branch, parallelize, retry, and stream operations. This page explains the design and the key types.

## The Runnable Trait

At the heart of LCEL is a single trait:

```rust
#[async_trait]
pub trait Runnable<I, O>: Send + Sync
where
    I: Send + 'static,
    O: Send + 'static,
{
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapticError>;

    async fn batch(&self, inputs: Vec<I>, config: &RunnableConfig) -> Vec<Result<O, SynapticError>>;

    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>;

    fn boxed(self) -> BoxRunnable<I, O>;
}
```

Only `invoke()` is required. Default implementations are provided for:
- `batch()` -- runs `invoke()` sequentially for each input
- `stream()` -- wraps `invoke()` as a single-item stream
- `boxed()` -- wraps `self` into a type-erased `BoxRunnable`

The `RunnableConfig` parameter threads runtime configuration (tags, metadata, concurrency limits, run IDs) through the entire pipeline without changing the input/output types.

## BoxRunnable and the Pipe Operator

Rust's type system requires concrete types for composition, but LCEL chains can contain heterogeneous steps. `BoxRunnable<I, O>` is a type-erased wrapper that erases the concrete type while preserving the `Runnable` interface.

The pipe operator (`|`) connects two boxed runnables into a `RunnableSequence`:

```rust
use synaptic::runnables::{BoxRunnable, Runnable, RunnableLambda};

let step1 = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
}).boxed();

let step2 = RunnableLambda::new(|x: String| async move {
    Ok(format!("Result: {x}"))
}).boxed();

let chain = step1 | step2;
let output = chain.invoke("hello".into(), &config).await?;
// output: "Result: HELLO"
```

This is Rust's `BitOr` trait overloaded on `BoxRunnable`. The intermediate type between steps must match -- the output of step1 must be the input type of step2.

## Key Runnable Types

### RunnablePassthrough

Passes input through unchanged. Useful as a branch in `RunnableParallel` or as a placeholder in a chain:

```rust
let passthrough = RunnablePassthrough::new().boxed();
// invoke("hello") => Ok("hello")
```

### RunnableLambda

Wraps an async closure into a `Runnable`. This is the most common way to insert custom logic into a chain:

```rust
let transform = RunnableLambda::new(|input: String| async move {
    Ok(input.split_whitespace().count())
}).boxed();
```

### RunnableSequence

Created by the `|` operator. Executes steps in order, feeding each output as the next step's input. You rarely construct this directly.

### RunnableParallel

Runs named branches concurrently and merges their outputs into a `serde_json::Value` object:

```rust
let parallel = RunnableParallel::new()
    .add("upper", RunnableLambda::new(|s: String| async move {
        Ok(Value::String(s.to_uppercase()))
    }).boxed())
    .add("length", RunnableLambda::new(|s: String| async move {
        Ok(Value::Number(s.len().into()))
    }).boxed());

let result = parallel.invoke("hello".into(), &config).await?;
// result: {"upper": "HELLO", "length": 5}
```

All branches receive a clone of the same input and run concurrently via `tokio::join!`. The output is a JSON object keyed by the branch names.

### RunnableBranch

Routes input to one of several branches based on conditions, with a default fallthrough:

```rust
let branch = RunnableBranch::new(
    vec![
        (
            |input: &String| input.starts_with("math:"),
            math_chain.boxed(),
        ),
        (
            |input: &String| input.starts_with("code:"),
            code_chain.boxed(),
        ),
    ],
    default_chain.boxed(),  // fallback
);
```

Conditions are checked in order. The first matching condition's branch is invoked. If none match, the default branch handles it.

### RunnableWithFallbacks

Tries alternatives when the primary runnable fails:

```rust
let robust = RunnableWithFallbacks::new(
    primary_model.boxed(),
    vec![fallback_model.boxed()],
);
```

If `primary_model` returns an error, `fallback_model` is tried with the same input. This is useful for model failover (e.g., try GPT-4, fall back to GPT-3.5).

### RunnableAssign

Runs a parallel branch and merges its output into the existing JSON value. The input must be a `serde_json::Value` object, and the parallel branch's outputs are merged as additional keys:

```rust
let assign = RunnableAssign::new(
    RunnableParallel::new()
        .add("word_count", count_words_runnable)
);
// Input: {"text": "hello world"}
// Output: {"text": "hello world", "word_count": 2}
```

### RunnablePick

Extracts specific keys from a JSON value:

```rust
let pick = RunnablePick::new(vec!["name".into(), "age".into()]);
// Input: {"name": "Alice", "age": 30, "email": "..."}
// Output: {"name": "Alice", "age": 30}
```

Single-key picks return the value directly rather than wrapping it in an object.

### RunnableEach

Maps a runnable over each element of a collection:

```rust
let each = RunnableEach::new(transform_single_item.boxed());
// Input: vec!["a", "b", "c"]
// Output: vec![transformed_a, transformed_b, transformed_c]
```

### RunnableRetry

Retries a runnable on failure with configurable policy:

```rust
let retry = RunnableRetry::new(
    flaky_runnable.boxed(),
    RetryPolicy {
        max_retries: 3,
        delay: Duration::from_millis(100),
        backoff_factor: 2.0,
    },
);
```

### RunnableGenerator

Produces values from a stream, useful for wrapping streaming sources into the runnable pipeline:

```rust
let generator = RunnableGenerator::new(|input: String, _config| {
    Box::pin(async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_string());
        }
    })
});
```

## Config Binding

`BoxRunnable::bind()` applies a config transform before delegation. This lets you attach metadata, set concurrency limits, or override run names without changing the chain's input/output types:

```rust
let tagged = chain.bind(|mut config| {
    config.tags.push("production".into());
    config
});
```

`with_config()` is a convenience that replaces the config entirely. `with_listeners()` adds before/after callbacks around invocation.

## Streaming Through Pipelines

When you call `stream()` on a chain, the streaming behavior depends on the components:

- If the **final** component in a sequence truly streams (e.g., an LLM that yields token-by-token), the chain streams those chunks through.
- Intermediate steps in the pipeline run their `invoke()` and pass the result forward.
- `RunnableGenerator` produces a true stream from any async function.

This means a chain like `prompt | model | parser` will stream the model's output chunks through the parser, provided the parser implements true streaming.

## Everything Is a Runnable

Synaptic's LCEL design means that many types across the framework implement `Runnable`:

- **Prompt templates** (`ChatPromptTemplate`) implement `Runnable<Value, Vec<Message>>` -- they take template variables and produce messages.
- **Output parsers** (`StrOutputParser`, `JsonOutputParser`, etc.) implement `Runnable` -- they transform one output format to another.
- **Chat models** can be wrapped as runnables for use in chains.
- **Graphs** produce state from state.

This uniformity means you can compose any of these with `|` and get type-safe, streamable pipelines.

## See Also

- [Pipe Operator](../how-to/runnables/pipe-operator.md) -- composing runnables with `|`
- [Streaming](../how-to/runnables/streaming.md) -- streaming through chains
- [Parallel & Branch](../how-to/runnables/parallel-branch.md) -- concurrent execution and routing
- [Assign & Pick](../how-to/runnables/assign-pick.md) -- JSON manipulation in chains
- [Fallbacks](../how-to/runnables/fallbacks.md) -- error recovery
- [Retry](../how-to/runnables/retry.md) -- automatic retry with backoff
- [Streaming (concept)](streaming.md) -- streaming across all layers
