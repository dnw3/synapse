use synaptic_eval::{EvalCase, EvalReport};

#[test]
fn computes_accuracy() {
    let report = EvalReport::from_cases(vec![
        EvalCase::new("a", "a"),
        EvalCase::new("b", "x"),
        EvalCase::new("c", "c"),
    ]);

    assert_eq!(report.total, 3);
    assert_eq!(report.passed, 2);
    assert!((report.accuracy - 0.666_666_7).abs() < 0.000_1);
}

#[test]
fn empty_cases_report() {
    let report = EvalReport::from_cases(vec![]);
    assert_eq!(report.total, 0);
    assert_eq!(report.passed, 0);
    assert!(report.accuracy.is_nan() || report.accuracy == 0.0);
}

#[test]
fn all_matching_cases() {
    let report = EvalReport::from_cases(vec![
        EvalCase::new("a", "a"),
        EvalCase::new("b", "b"),
        EvalCase::new("c", "c"),
    ]);
    assert_eq!(report.total, 3);
    assert_eq!(report.passed, 3);
    assert!((report.accuracy - 1.0).abs() < 1e-6);
}
