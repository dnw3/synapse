# Evaluators

Synaptic provides five built-in evaluators, ranging from simple string matching to LLM-based judgment. All implement the `Evaluator` trait and return an `EvalResult` with a score, pass/fail status, and optional reasoning.

## ExactMatchEvaluator

Checks whether the prediction exactly matches the reference string:

```rust
use synaptic::eval::{ExactMatchEvaluator, Evaluator};

// Case-sensitive (default)
let eval = ExactMatchEvaluator::new();
let result = eval.evaluate("hello", "hello", "").await?;
assert!(result.passed);
assert_eq!(result.score, 1.0);

let result = eval.evaluate("Hello", "hello", "").await?;
assert!(!result.passed);  // Case mismatch

// Case-insensitive
let eval = ExactMatchEvaluator::case_insensitive();
let result = eval.evaluate("Hello", "hello", "").await?;
assert!(result.passed);  // Now passes
```

On failure, the reasoning field shows what was expected versus what was received.

## JsonValidityEvaluator

Checks whether the prediction is valid JSON. The reference and input are ignored:

```rust
use synaptic::eval::{JsonValidityEvaluator, Evaluator};

let eval = JsonValidityEvaluator::new();

let result = eval.evaluate(r#"{"key": "value"}"#, "", "").await?;
assert!(result.passed);

let result = eval.evaluate("not json", "", "").await?;
assert!(!result.passed);
// reasoning: "Invalid JSON: expected ident at line 1 column 2"
```

This is useful for validating that an LLM produced well-formed JSON output.

## RegexMatchEvaluator

Checks whether the prediction matches a regular expression pattern:

```rust
use synaptic::eval::{RegexMatchEvaluator, Evaluator};

// Match a date pattern
let eval = RegexMatchEvaluator::new(r"\d{4}-\d{2}-\d{2}")?;

let result = eval.evaluate("2024-01-15", "", "").await?;
assert!(result.passed);

let result = eval.evaluate("January 15, 2024", "", "").await?;
assert!(!result.passed);
```

The constructor returns a `Result` because the regex pattern is validated at creation time. Invalid patterns produce a `SynapticError::Validation`.

## EmbeddingDistanceEvaluator

Computes cosine similarity between the embeddings of the prediction and reference. The score equals the cosine similarity, and the evaluation passes if the similarity meets or exceeds the threshold:

```rust
use synaptic::eval::{EmbeddingDistanceEvaluator, Evaluator};
use synaptic::embeddings::FakeEmbeddings;
use std::sync::Arc;

let embeddings = Arc::new(FakeEmbeddings::new());
let eval = EmbeddingDistanceEvaluator::new(embeddings, 0.8);

let result = eval.evaluate("the cat sat", "the cat sat on the mat", "").await?;
println!("Similarity: {:.4}", result.score);
println!("Passed (>= 0.8): {}", result.passed);
// reasoning: "Cosine similarity: 0.9234, threshold: 0.8000"
```

Parameters:

- **`embeddings`** -- any type implementing `Arc<dyn Embeddings>` (e.g., `OpenAiEmbeddings` from `synaptic::openai`, `OllamaEmbeddings` from `synaptic::ollama`, `FakeEmbeddings` from `synaptic::embeddings`).
- **`threshold`** -- minimum cosine similarity to pass. A typical value is `0.8` for semantic similarity checks.

## LLMJudgeEvaluator

Uses an LLM to judge the quality of a prediction on a 0-10 scale. The score is normalized to 0.0-1.0:

```rust
use synaptic::eval::{LLMJudgeEvaluator, Evaluator};
use synaptic::openai::OpenAiChatModel;
use std::sync::Arc;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let eval = LLMJudgeEvaluator::new(model);

let result = eval.evaluate(
    "Paris is the capital of France.",  // prediction
    "The capital of France is Paris.",  // reference
    "What is the capital of France?",   // input
).await?;

println!("Score: {:.1}/10", result.score * 10.0);
// reasoning: "LLM judge score: 9.0/10"
```

### Custom Prompt Template

You can customize the judge prompt. The template must contain `{input}`, `{prediction}`, and `{reference}` placeholders:

```rust
let eval = LLMJudgeEvaluator::with_prompt(
    model,
    r#"Evaluate whether the response is factually accurate.

Question: {input}
Expected: {reference}
Response: {prediction}

Rate accuracy from 0 (wrong) to 10 (perfect). Reply with a single number."#,
);
```

The default prompt asks the LLM to rate overall quality. The response is parsed for a number between 0 and 10; if no valid number is found, the evaluator returns a `SynapticError::Parsing`.

## Summary

| Evaluator | Speed | Requires |
|-----------|-------|----------|
| `ExactMatchEvaluator` | Instant | Nothing |
| `JsonValidityEvaluator` | Instant | Nothing |
| `RegexMatchEvaluator` | Instant | Nothing |
| `EmbeddingDistanceEvaluator` | Fast | Embeddings model |
| `LLMJudgeEvaluator` | Slow (LLM call) | Chat model |
