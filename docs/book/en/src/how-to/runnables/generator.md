# Generator

This guide shows how to use `RunnableGenerator` to create a runnable from a streaming generator function.

## Overview

`RunnableGenerator` wraps a function that produces a `Stream` of results. It bridges the gap between streaming generators and the `Runnable` trait:

- **`invoke()`** collects the entire stream into a `Vec<O>`
- **`stream()`** yields each item individually as it is produced

This is useful when you want a runnable that naturally produces output incrementally -- for example, tokenizers, chunkers, or any computation that yields partial results.

## Basic usage

```rust
use synaptic_runnables::{Runnable, RunnableGenerator};
use synaptic_core::{RunnableConfig, SynapticError};

let gen = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_uppercase());
        }
    }
});

let config = RunnableConfig::default();
let result = gen.invoke("hello world".to_string(), &config).await?;
assert_eq!(result, vec!["HELLO", "WORLD"]);
```

## Streaming

The real power of `RunnableGenerator` is streaming. `stream()` yields each item as it is produced, without waiting for the generator to finish:

```rust
use futures::StreamExt;
use synaptic_runnables::{Runnable, RunnableGenerator};
use synaptic_core::RunnableConfig;

let gen = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for ch in input.chars() {
            yield Ok(ch.to_string());
        }
    }
});

let config = RunnableConfig::default();
let mut stream = gen.stream("abc".to_string(), &config);

// Each item arrives individually wrapped in a Vec
while let Some(item) = stream.next().await {
    let chunk = item?;
    println!("{:?}", chunk); // ["a"], ["b"], ["c"]
}
```

Each streamed item is wrapped in `Vec<O>` to match the output type of `invoke()`. This means `stream()` yields `Result<Vec<O>, SynapticError>` where each `Vec` contains a single element.

## Error handling

If the generator yields an `Err`, `invoke()` stops collecting and returns that error. `stream()` yields the error and continues to the next item:

```rust
use synaptic_runnables::RunnableGenerator;
use synaptic_core::SynapticError;

let gen = RunnableGenerator::new(|_input: String| {
    async_stream::stream! {
        yield Ok("first".to_string());
        yield Err(SynapticError::Other("oops".into()));
        yield Ok("third".to_string());
    }
});

// invoke() fails on the error:
// gen.invoke("x".to_string(), &config).await => Err(...)

// stream() yields all three items:
// Ok(["first"]), Err(...), Ok(["third"])
```

## In a pipeline

`RunnableGenerator` implements `Runnable<I, Vec<O>>`, so it works with the pipe operator. Place it wherever you need streaming generation in a chain:

```rust
use synaptic_runnables::{Runnable, RunnableGenerator, RunnableLambda};

let tokenize = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for token in input.split_whitespace() {
            yield Ok(token.to_string());
        }
    }
});

let count = RunnableLambda::new(|tokens: Vec<String>| async move {
    Ok(tokens.len())
});

let chain = tokenize.boxed() | count.boxed();

// chain.invoke("one two three".to_string(), &config).await => Ok(3)
```

## Type signature

```rust,ignore
pub struct RunnableGenerator<I: Send + 'static, O: Send + 'static> { ... }

impl<I, O> Runnable<I, Vec<O>> for RunnableGenerator<I, O> { ... }
```

The constructor accepts any function `Fn(I) -> S` where `S: Stream<Item = Result<O, SynapticError>> + Send + 'static`. The `async_stream::stream!` macro is the most ergonomic way to produce such a stream.
