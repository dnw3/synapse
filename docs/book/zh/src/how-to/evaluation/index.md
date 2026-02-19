# Evaluation

Synaptic 提供了一个评估框架，用于衡量 AI 输出的质量。`Evaluator` trait 定义了一个标准接口，用于对预测结果与参考答案进行评分，而 `Dataset` + `evaluate()` 管道使得跨多个测试用例运行批量评估变得简单。

## `Evaluator` Trait

所有 Evaluator 都实现了 `synaptic_eval` 中的 `Evaluator` trait：

```rust
#[async_trait]
pub trait Evaluator: Send + Sync {
    async fn evaluate(
        &self,
        prediction: &str,
        reference: &str,
        input: &str,
    ) -> Result<EvalResult, SynapticError>;
}
```

- **`prediction`** -- 待评估的 AI 输出。
- **`reference`** -- 预期的或真实的答案。
- **`input`** -- 产生预测结果的原始输入。

## `EvalResult`

每个 Evaluator 返回一个 `EvalResult`：

```rust
pub struct EvalResult {
    pub score: f64,       // Between 0.0 and 1.0
    pub passed: bool,     // true if score >= 0.5
    pub reasoning: Option<String>,  // Optional explanation
}
```

辅助构造方法：

| 方法 | 分数 | 是否通过 |
|------|------|----------|
| `EvalResult::pass()` | 1.0 | true |
| `EvalResult::fail()` | 0.0 | false |
| `EvalResult::with_score(0.75)` | 0.75 | true (>= 0.5) |

你可以通过 `.with_reasoning("explanation")` 附加推理说明。

## 内置 Evaluator

Synaptic 开箱提供五种 Evaluator：

| Evaluator | 检查内容 |
|-----------|----------|
| `ExactMatchEvaluator` | 精确字符串匹配（可选大小写不敏感模式） |
| `JsonValidityEvaluator` | 预测结果是否为有效 JSON |
| `RegexMatchEvaluator` | 预测结果是否匹配正则表达式模式 |
| `EmbeddingDistanceEvaluator` | 预测结果与参考答案的 Embedding 余弦相似度 |
| `LLMJudgeEvaluator` | 使用 LLM 对预测质量进行 0-10 分评分 |

详细用法请参见 [Evaluators](evaluators.md)。

## 批量评估

`evaluate()` 函数在 `Dataset` 测试用例上运行 Evaluator，生成包含聚合统计信息的 `EvalReport`。详情请参见 [Datasets](dataset.md)。

## 指南

- [Evaluators](evaluators.md) -- 每个内置 Evaluator 的用法和配置
- [Datasets](dataset.md) -- 使用 `Dataset` 和 `evaluate()` 进行批量评估
