use synaptic_flashrank::FlashRankConfig;

#[test]
fn default_config() {
    let c = FlashRankConfig::default();
    assert!((c.k1 - 1.5).abs() < f32::EPSILON);
    assert!((c.b - 0.75).abs() < f32::EPSILON);
}

#[test]
fn builder_pattern() {
    let c = FlashRankConfig::new().with_k1(2.0).with_b(0.5);
    assert!((c.k1 - 2.0).abs() < f32::EPSILON);
    assert!((c.b - 0.5).abs() < f32::EPSILON);
}

#[test]
fn builder_k1_zero() {
    let c = FlashRankConfig::new().with_k1(0.0);
    assert!((c.k1 - 0.0).abs() < f32::EPSILON);
}
