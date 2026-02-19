# Each

This guide shows how to use `RunnableEach` to map a runnable over each element in a list.

## Overview

`RunnableEach` wraps any `BoxRunnable<I, O>` and applies it to every element in a `Vec<I>`, producing a `Vec<O>`. It is the runnable equivalent of `Iterator::map()` -- process a batch of items through the same transformation.

## Basic usage

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::RunnableConfig;

let upper = RunnableLambda::new(|s: String| async move {
    Ok(s.to_uppercase())
});

let each = RunnableEach::new(upper.boxed());

let config = RunnableConfig::default();
let result = each.invoke(
    vec!["hello".into(), "world".into()],
    &config,
).await?;

assert_eq!(result, vec!["HELLO", "WORLD"]);
```

## Error propagation

If the inner runnable fails on any element, `RunnableEach` stops and returns that error immediately. Elements processed before the failure are discarded:

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::{RunnableConfig, SynapticError};

let must_be_short = RunnableLambda::new(|s: String| async move {
    if s.len() > 5 {
        Err(SynapticError::Other(format!("too long: {s}")))
    } else {
        Ok(s.to_uppercase())
    }
});

let each = RunnableEach::new(must_be_short.boxed());
let config = RunnableConfig::default();

let result = each.invoke(
    vec!["hi".into(), "toolong".into(), "ok".into()],
    &config,
).await;

assert!(result.is_err()); // fails on "toolong"
```

## Empty input

An empty input vector produces an empty output vector:

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::RunnableConfig;

let identity = RunnableLambda::new(|s: String| async move { Ok(s) });
let each = RunnableEach::new(identity.boxed());

let config = RunnableConfig::default();
let result = each.invoke(vec![], &config).await?;
assert!(result.is_empty());
```

## In a pipeline

`RunnableEach` implements `Runnable<Vec<I>, Vec<O>>`, so it composes with the pipe operator. A common pattern is to split input into parts, process each with `RunnableEach`, and then combine the results:

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};

// Step 1: split a string into words
let split = RunnableLambda::new(|s: String| async move {
    Ok(s.split_whitespace().map(String::from).collect::<Vec<_>>())
});

// Step 2: process each word
let process = RunnableEach::new(
    RunnableLambda::new(|w: String| async move {
        Ok(w.to_uppercase())
    }).boxed()
);

// Step 3: join results
let join = RunnableLambda::new(|words: Vec<String>| async move {
    Ok(words.join(", "))
});

let chain = split.boxed() | process.boxed() | join.boxed();
// chain.invoke("hello world".to_string(), &config).await => Ok("HELLO, WORLD")
```

## Type signature

```rust,ignore
pub struct RunnableEach<I: Send + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
}

impl<I, O> Runnable<Vec<I>, Vec<O>> for RunnableEach<I, O> { ... }
```

Elements are processed sequentially in order. For concurrent processing, use `RunnableParallel` or the `batch()` method on a `BoxRunnable` instead.
