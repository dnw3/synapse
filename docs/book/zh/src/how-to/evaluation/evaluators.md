# Evaluators

Synaptic 提供五种内置 Evaluator，从简单的字符串匹配到基于 LLM 的判断。所有 Evaluator 都实现了 `Evaluator` trait，并返回包含分数、通过/失败状态和可选推理说明的 `EvalResult`。

## ExactMatchEvaluator

检查预测结果是否与参考字符串完全匹配：

```rust
use synaptic_eval::{ExactMatchEvaluator, Evaluator};

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

匹配失败时，reasoning 字段会显示期望值与实际值的对比。

## JsonValidityEvaluator

检查预测结果是否为有效 JSON。reference 和 input 参数会被忽略：

```rust
use synaptic_eval::{JsonValidityEvaluator, Evaluator};

let eval = JsonValidityEvaluator::new();

let result = eval.evaluate(r#"{"key": "value"}"#, "", "").await?;
assert!(result.passed);

let result = eval.evaluate("not json", "", "").await?;
assert!(!result.passed);
// reasoning: "Invalid JSON: expected ident at line 1 column 2"
```

这对于验证 LLM 是否生成了格式正确的 JSON 输出非常有用。

## RegexMatchEvaluator

检查预测结果是否匹配正则表达式模式：

```rust
use synaptic_eval::{RegexMatchEvaluator, Evaluator};

// Match a date pattern
let eval = RegexMatchEvaluator::new(r"\d{4}-\d{2}-\d{2}")?;

let result = eval.evaluate("2024-01-15", "", "").await?;
assert!(result.passed);

let result = eval.evaluate("January 15, 2024", "", "").await?;
assert!(!result.passed);
```

构造函数返回 `Result`，因为正则表达式模式在创建时会被验证。无效的模式会产生 `SynapticError::Validation`。

## EmbeddingDistanceEvaluator

计算预测结果和参考答案的 Embedding 之间的余弦相似度。分数等于余弦相似度，如果相似度达到或超过阈值，则评估通过：

```rust
use synaptic_eval::{EmbeddingDistanceEvaluator, Evaluator};
use synaptic_embeddings::FakeEmbeddings;
use std::sync::Arc;

let embeddings = Arc::new(FakeEmbeddings::new());
let eval = EmbeddingDistanceEvaluator::new(embeddings, 0.8);

let result = eval.evaluate("the cat sat", "the cat sat on the mat", "").await?;
println!("Similarity: {:.4}", result.score);
println!("Passed (>= 0.8): {}", result.passed);
// reasoning: "Cosine similarity: 0.9234, threshold: 0.8000"
```

参数：

- **`embeddings`** -- 任何实现了 `Arc<dyn Embeddings>` 的类型（例如 `OpenAiEmbeddings`、`OllamaEmbeddings`、`FakeEmbeddings`）。
- **`threshold`** -- 通过所需的最小余弦相似度。语义相似性检查的典型值为 `0.8`。

## LLMJudgeEvaluator

使用 LLM 对预测质量在 0-10 分制上进行评判。分数被归一化到 0.0-1.0：

```rust
use synaptic_eval::{LLMJudgeEvaluator, Evaluator};
use synaptic_models::OpenAiChatModel;
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

### 自定义 Prompt 模板

你可以自定义评判 Prompt。模板必须包含 `{input}`、`{prediction}` 和 `{reference}` 占位符：

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

默认 Prompt 要求 LLM 对整体质量进行评分。响应会被解析以提取 0 到 10 之间的数字；如果未找到有效数字，Evaluator 会返回 `SynapticError::Parsing`。

## 总结

| Evaluator | 速度 | 依赖 |
|-----------|------|------|
| `ExactMatchEvaluator` | 即时 | 无 |
| `JsonValidityEvaluator` | 即时 | 无 |
| `RegexMatchEvaluator` | 即时 | 无 |
| `EmbeddingDistanceEvaluator` | 快速 | Embeddings 模型 |
| `LLMJudgeEvaluator` | 较慢（LLM 调用） | Chat 模型 |
