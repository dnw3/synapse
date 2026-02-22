use synaptic_voyage::{VoyageConfig, VoyageModel};

#[test]
fn test_config_default_base_url() {
    let config = VoyageConfig::new("key", VoyageModel::Voyage3Large);
    assert_eq!(config.base_url, "https://api.voyageai.com/v1");
}

#[test]
fn test_config_custom_base_url() {
    let config = VoyageConfig::new("key", VoyageModel::Voyage3)
        .with_base_url("https://custom.endpoint.com/v1");
    assert_eq!(config.base_url, "https://custom.endpoint.com/v1");
}

#[test]
fn test_all_model_variants() {
    let models = vec![
        (VoyageModel::Voyage3Large, "voyage-3-large"),
        (VoyageModel::Voyage3, "voyage-3"),
        (VoyageModel::Voyage3Lite, "voyage-3-lite"),
        (VoyageModel::VoyageCode3, "voyage-code-3"),
        (VoyageModel::VoyageFinance2, "voyage-finance-2"),
    ];
    for (model, expected) in models {
        assert_eq!(model.as_str(), expected);
    }
}
