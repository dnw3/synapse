use synaptic_fireworks::{FireworksConfig, FireworksModel};
use synaptic_openai::OpenAiConfig;

#[test]
fn test_model_as_str() {
    assert_eq!(
        FireworksModel::Llama3_1_70bInstruct.as_str(),
        "accounts/fireworks/models/llama-v3p1-70b-instruct"
    );
    assert_eq!(
        FireworksModel::Llama3_1_8bInstruct.as_str(),
        "accounts/fireworks/models/llama-v3p1-8b-instruct"
    );
    assert_eq!(
        FireworksModel::DeepSeekR1.as_str(),
        "accounts/fireworks/models/deepseek-r1"
    );
    assert_eq!(
        FireworksModel::Qwen2_5_72bInstruct.as_str(),
        "accounts/fireworks/models/qwen2p5-72b-instruct"
    );
    assert_eq!(
        FireworksModel::Custom("my/model".into()).as_str(),
        "my/model"
    );
}

#[test]
fn test_config_builder() {
    let config = FireworksConfig::new("key", FireworksModel::Llama3_1_70bInstruct)
        .with_temperature(0.8)
        .with_max_tokens(2048)
        .with_top_p(0.95);
    assert_eq!(config.temperature, Some(0.8));
    assert_eq!(config.max_tokens, Some(2048));
    assert_eq!(config.top_p, Some(0.95));
}

#[test]
fn test_config_into_openai() {
    let cfg: OpenAiConfig = FireworksConfig::new("fw-key", FireworksModel::Llama3_1_70bInstruct)
        .with_temperature(0.7)
        .into();
    assert_eq!(cfg.base_url, "https://api.fireworks.ai/inference/v1");
    assert_eq!(cfg.temperature, Some(0.7));
}

#[test]
fn test_model_display() {
    assert_eq!(
        format!("{}", FireworksModel::DeepSeekR1),
        "accounts/fireworks/models/deepseek-r1"
    );
}
