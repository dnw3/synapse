use synaptic_nomic::{NomicConfig, NomicModel};

#[test]
fn test_default_model() {
    let config = NomicConfig::new("key");
    assert_eq!(config.model, "nomic-embed-text-v1.5");
}

#[test]
fn test_custom_model() {
    let config = NomicConfig::new("key").with_model(NomicModel::Custom("my-model".into()));
    assert_eq!(config.model, "my-model");
}

#[test]
fn test_model_display() {
    assert_eq!(
        format!("{}", NomicModel::NomicEmbedTextV1_5),
        "nomic-embed-text-v1.5"
    );
}
