use synaptic_openai::OpenAiConfig;
use synaptic_together::{TogetherConfig, TogetherModel};

#[test]
fn test_model_as_str() {
    assert_eq!(
        TogetherModel::Llama3_3_70bInstructTurbo.as_str(),
        "meta-llama/Llama-3.3-70B-Instruct-Turbo"
    );
    assert_eq!(
        TogetherModel::Llama3_1_8bInstructTurbo.as_str(),
        "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo"
    );
    assert_eq!(
        TogetherModel::DeepSeekR1.as_str(),
        "deepseek-ai/DeepSeek-R1"
    );
    assert_eq!(
        TogetherModel::Qwen2_5_72bInstructTurbo.as_str(),
        "Qwen/Qwen2.5-72B-Instruct-Turbo"
    );
    assert_eq!(
        TogetherModel::Custom("my-model".into()).as_str(),
        "my-model"
    );
}

#[test]
fn test_config_builder() {
    let config = TogetherConfig::new("key", TogetherModel::Llama3_3_70bInstructTurbo)
        .with_temperature(0.7)
        .with_max_tokens(1024)
        .with_top_p(0.9);
    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.max_tokens, Some(1024));
    assert_eq!(config.top_p, Some(0.9));
}

#[test]
fn test_config_into_openai() {
    let cfg: OpenAiConfig = TogetherConfig::new("key", TogetherModel::Llama3_3_70bInstructTurbo)
        .with_temperature(0.5)
        .into();
    assert_eq!(cfg.base_url, "https://api.together.xyz/v1");
    assert_eq!(cfg.temperature, Some(0.5));
}

#[test]
fn test_model_display() {
    assert_eq!(
        format!("{}", TogetherModel::DeepSeekR1),
        "deepseek-ai/DeepSeek-R1"
    );
}
