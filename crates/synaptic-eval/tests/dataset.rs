use synaptic_eval::{evaluate, Dataset, ExactMatchEvaluator};

#[tokio::test]
async fn evaluate_dataset() {
    let dataset = Dataset::from_pairs(vec![("q1", "Paris"), ("q2", "Berlin"), ("q3", "Tokyo")]);

    let predictions = vec![
        "Paris".to_string(),
        "London".to_string(),
        "Tokyo".to_string(),
    ];

    let evaluator = ExactMatchEvaluator::new();
    let report = evaluate(&evaluator, &dataset, &predictions).await.unwrap();

    assert_eq!(report.total, 3);
    assert_eq!(report.passed, 2);
    assert!((report.accuracy - 2.0 / 3.0).abs() < 0.001);
    assert_eq!(report.results.len(), 3);
    assert!(report.results[0].passed);
    assert!(!report.results[1].passed);
    assert!(report.results[2].passed);
}

#[tokio::test]
async fn empty_dataset() {
    let dataset = Dataset::new(vec![]);
    let predictions: Vec<String> = vec![];

    let evaluator = ExactMatchEvaluator::new();
    let report = evaluate(&evaluator, &dataset, &predictions).await.unwrap();

    assert_eq!(report.total, 0);
    assert_eq!(report.passed, 0);
    assert_eq!(report.accuracy, 0.0);
    assert!(report.results.is_empty());
}
