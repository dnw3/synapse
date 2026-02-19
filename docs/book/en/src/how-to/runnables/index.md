# Runnables (LCEL)

Synaptic implements LCEL (LangChain Expression Language) through the `Runnable` trait and a set of composable building blocks. Every component in an LCEL chain -- prompts, models, parsers, custom logic -- implements the same `Runnable<I, O>` interface, so they can be combined freely with a uniform API.

## The `Runnable` trait

The `Runnable<I, O>` trait is defined in `synaptic_core` and provides three core methods:

| Method | Description |
|--------|-------------|
| `invoke(input, config)` | Execute on a single input, returning one output |
| `batch(inputs, config)` | Execute on multiple inputs sequentially |
| `stream(input, config)` | Return a `RunnableOutputStream` of incremental results |

Every `Runnable` also has a `boxed()` method that wraps it into a `BoxRunnable<I, O>` -- a type-erased container that enables the `|` pipe operator for composition.

```rust
use synaptic_runnables::{Runnable, RunnableLambda, BoxRunnable};
use synaptic_core::RunnableConfig;

let step = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
});

let config = RunnableConfig::default();
let result = step.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "HELLO");
```

## `BoxRunnable` -- type-erased composition

`BoxRunnable<I, O>` is the key type for building chains. It wraps any `Runnable<I, O>` behind a trait object, which erases the concrete type. This is necessary because the `|` operator requires both sides to have known types at the call site.

`BoxRunnable` itself implements `Runnable<I, O>`, so boxed runnables compose seamlessly.

## Building blocks

Synaptic provides the following LCEL building blocks:

| Type | Purpose |
|------|---------|
| `RunnableLambda` | Wraps an async closure as a runnable |
| `RunnablePassthrough` | Passes input through unchanged |
| `RunnableSequence` | Chains two runnables (created by `\|` operator) |
| `RunnableParallel` | Runs named branches concurrently, merges to JSON |
| `RunnableBranch` | Routes input by condition, with a default fallback |
| `RunnableAssign` | Merges parallel branch results into the input JSON object |
| `RunnablePick` | Extracts specific keys from a JSON object |
| `RunnableWithFallbacks` | Tries alternatives when the primary runnable fails |
| `RunnableRetry` | Retries with exponential backoff on failure |
| `RunnableEach` | Maps a runnable over each element in a `Vec` |
| `RunnableGenerator` | Wraps a generator function for true streaming output |

## Guides

- [Pipe Operator](pipe-operator.md) -- chain runnables with `|` to build sequential pipelines
- [Streaming](streaming.md) -- consume incremental output through a chain
- [Parallel & Branch](parallel-branch.md) -- run branches concurrently or route by condition
- [Assign & Pick](assign-pick.md) -- merge computed keys into JSON and extract specific fields
- [Fallbacks](fallbacks.md) -- provide alternative runnables when the primary one fails
- [Bind](bind.md) -- attach config transforms to a runnable
- [Retry](retry.md) -- retry with exponential backoff on transient failures
- [Generator](generator.md) -- wrap a streaming generator function as a runnable
- [Each](each.md) -- map a runnable over each element in a list
