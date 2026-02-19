use synaptic::core::SynapticError;
use synaptic::eval::{evaluate, Dataset, Evaluator, ExactMatchEvaluator};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // --- Single evaluation ---
    println!("=== Single Evaluation ===");
    let evaluator = ExactMatchEvaluator::new();

    let result = evaluator
        .evaluate("Paris", "Paris", "What is the capital of France?")
        .await?;
    println!("Exact match (Paris vs Paris): score={}", result.score);

    let result = evaluator
        .evaluate("paris", "Paris", "What is the capital of France?")
        .await?;
    println!("Case mismatch (paris vs Paris): score={}", result.score);

    // --- Case-insensitive evaluator ---
    println!("\n=== Case-Insensitive ===");
    let ci_evaluator = ExactMatchEvaluator::case_insensitive();
    let result = ci_evaluator
        .evaluate("paris", "Paris", "What is the capital of France?")
        .await?;
    println!("Case-insensitive (paris vs Paris): score={}", result.score);

    // --- Batch evaluation with Dataset ---
    println!("\n=== Batch Evaluation ===");
    let dataset = Dataset::from_pairs(vec![
        ("What is 2+2?", "4"),
        ("Capital of France?", "Paris"),
        ("Largest planet?", "Jupiter"),
        ("Rust creator?", "Graydon Hoare"),
    ]);

    let predictions = vec![
        "4".to_string(),
        "Paris".to_string(),
        "Saturn".to_string(), // wrong
        "Graydon Hoare".to_string(),
    ];

    let report = evaluate(&evaluator, &dataset, &predictions).await?;
    println!(
        "Results: {}/{} correct",
        report.results.iter().filter(|r| r.score == 1.0).count(),
        report.results.len()
    );
    for (i, result) in report.results.iter().enumerate() {
        let input = &dataset.items[i].input;
        let status = if result.score == 1.0 { "PASS" } else { "FAIL" };
        println!(
            "  [{status}] {input}: predicted='{}', expected='{}'",
            predictions[i], dataset.items[i].reference
        );
    }

    println!("\nEvaluation demo completed successfully!");
    Ok(())
}
