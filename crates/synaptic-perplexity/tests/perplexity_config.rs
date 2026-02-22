use synaptic_openai::OpenAiConfig;
use synaptic_perplexity::{PerplexityConfig, PerplexityModel};

#[test]
fn test_model_as_str() {
    assert_eq!(PerplexityModel::SonarLarge.as_str(), "sonar-large-online");
    assert_eq!(PerplexityModel::SonarSmall.as_str(), "sonar-small-online");
    assert_eq!(PerplexityModel::SonarHuge.as_str(), "sonar-huge-online");
    assert_eq!(
        PerplexityModel::SonarReasoningPro.as_str(),
        "sonar-reasoning-pro"
    );
    assert_eq!(
        PerplexityModel::Custom("sonar-v2".into()).as_str(),
        "sonar-v2"
    );
}

#[test]
fn test_config_builder() {
    let config = PerplexityConfig::new("key", PerplexityModel::SonarLarge)
        .with_temperature(0.1)
        .with_max_tokens(2048);
    assert_eq!(config.temperature, Some(0.1));
    assert_eq!(config.max_tokens, Some(2048));
}

#[test]
fn test_config_into_openai() {
    let cfg: OpenAiConfig = PerplexityConfig::new("pplx-key", PerplexityModel::SonarLarge)
        .with_temperature(0.5)
        .into();
    assert_eq!(cfg.base_url, "https://api.perplexity.ai");
    assert_eq!(cfg.temperature, Some(0.5));
}

#[test]
fn test_model_display() {
    assert_eq!(
        format!("{}", PerplexityModel::SonarLarge),
        "sonar-large-online"
    );
    assert_eq!(
        format!("{}", PerplexityModel::SonarReasoningPro),
        "sonar-reasoning-pro"
    );
}
