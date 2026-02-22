use synaptic_openai::OpenAiConfig;
use synaptic_xai::{XaiConfig, XaiModel};

#[test]
fn test_model_as_str() {
    assert_eq!(XaiModel::Grok2Latest.as_str(), "grok-2-latest");
    assert_eq!(XaiModel::Grok2Mini.as_str(), "grok-2-mini");
    assert_eq!(XaiModel::GrokBeta.as_str(), "grok-beta");
    assert_eq!(XaiModel::Custom("grok-3".into()).as_str(), "grok-3");
}

#[test]
fn test_config_builder() {
    let config = XaiConfig::new("key", XaiModel::Grok2Latest)
        .with_temperature(0.7)
        .with_max_tokens(4096);
    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.max_tokens, Some(4096));
}

#[test]
fn test_config_into_openai() {
    let cfg: OpenAiConfig = XaiConfig::new("xai-key", XaiModel::Grok2Latest)
        .with_temperature(0.5)
        .into();
    assert_eq!(cfg.base_url, "https://api.x.ai/v1");
    assert_eq!(cfg.temperature, Some(0.5));
}

#[test]
fn test_model_display() {
    assert_eq!(format!("{}", XaiModel::Grok2Latest), "grok-2-latest");
}
