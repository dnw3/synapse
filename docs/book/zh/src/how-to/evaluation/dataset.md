# Datasets

`Dataset` 类型和 `evaluate()` 函数提供了批量评估管道。你定义一个包含输入-参考对的 Dataset，生成预测结果，然后一次性评分所有结果以生成 `EvalReport`。

## 创建 Dataset

`Dataset` 是 `DatasetItem` 值的集合，每个值包含一个 `input` 和一个 `reference`（预期答案）：

```rust
use synaptic_eval::{Dataset, DatasetItem};

// From DatasetItem structs
let dataset = Dataset::new(vec![
    DatasetItem {
        input: "What is 2+2?".to_string(),
        reference: "4".to_string(),
    },
    DatasetItem {
        input: "Capital of France?".to_string(),
        reference: "Paris".to_string(),
    },
]);

// From string pairs (convenience method)
let dataset = Dataset::from_pairs(vec![
    ("What is 2+2?", "4"),
    ("Capital of France?", "Paris"),
]);
```

## 运行批量评估

`evaluate()` 函数接收一个 Evaluator、一个 Dataset 和一个预测结果切片。它对每个预测结果与对应的 Dataset 项进行评估，并返回 `EvalReport`：

```rust
use synaptic_eval::{evaluate, Dataset, ExactMatchEvaluator};

let dataset = Dataset::from_pairs(vec![
    ("What is 2+2?", "4"),
    ("Capital of France?", "Paris"),
    ("Largest ocean?", "Pacific"),
]);

let evaluator = ExactMatchEvaluator::new();

// Your model's predictions (one per dataset item)
let predictions = vec![
    "4".to_string(),
    "Paris".to_string(),
    "Atlantic".to_string(),  // Wrong!
];

let report = evaluate(&evaluator, &dataset, &predictions).await?;

println!("Total: {}", report.total);      // 3
println!("Passed: {}", report.passed);     // 2
println!("Accuracy: {:.0}%", report.accuracy * 100.0);  // 67%
```

预测结果的数量必须与 Dataset 项的数量匹配。如果不匹配，`evaluate()` 返回 `SynapticError::Validation`。

## `EvalReport`

报告包含聚合统计信息和逐项结果：

```rust
pub struct EvalReport {
    pub total: usize,
    pub passed: usize,
    pub accuracy: f32,
    pub results: Vec<EvalResult>,
}
```

你可以检查各项结果以获取详细反馈：

```rust
for (i, result) in report.results.iter().enumerate() {
    let status = if result.passed { "PASS" } else { "FAIL" };
    let reason = result.reasoning.as_deref().unwrap_or("--");
    println!("[{status}] Item {i}: score={:.2}, reason={reason}", result.score);
}
```

## 端到端示例

典型的评估工作流：

1. 构建测试用例的 Dataset。
2. 在每个输入上运行你的模型/链以产生预测结果。
3. 使用 Evaluator 对预测结果进行评分。
4. 检查报告。

```rust
use synaptic_eval::{evaluate, Dataset, ExactMatchEvaluator};

// 1. Dataset
let dataset = Dataset::from_pairs(vec![
    ("2+2", "4"),
    ("3*5", "15"),
    ("10/2", "5"),
]);

// 2. Generate predictions (in practice, run your model)
let predictions: Vec<String> = dataset.items.iter()
    .map(|item| {
        // Simulated model output
        match item.input.as_str() {
            "2+2" => "4",
            "3*5" => "15",
            "10/2" => "5",
            _ => "unknown",
        }.to_string()
    })
    .collect();

// 3. Evaluate
let evaluator = ExactMatchEvaluator::new();
let report = evaluate(&evaluator, &dataset, &predictions).await?;

// 4. Report
println!("Accuracy: {:.0}% ({}/{})",
    report.accuracy * 100.0, report.passed, report.total);
```

## 使用不同的 Evaluator

`evaluate()` 函数可以与任何 `Evaluator` 配合使用。替换不同的 Evaluator 即可改变评分标准，无需修改 Dataset 或预测管道：

```rust
use synaptic_eval::{evaluate, RegexMatchEvaluator};

// Check that predictions contain a date
let evaluator = RegexMatchEvaluator::new(r"\d{4}-\d{2}-\d{2}")?;
let report = evaluate(&evaluator, &dataset, &predictions).await?;
```
