use std::path::Path;

use serde::Deserialize;
use synaptic_config::{parse_config, ConfigFormat};

// -- ConfigFormat::from_extension --

#[test]
fn format_from_extension_toml() {
    assert_eq!(
        ConfigFormat::from_extension("toml"),
        Some(ConfigFormat::Toml)
    );
    assert_eq!(
        ConfigFormat::from_extension("TOML"),
        Some(ConfigFormat::Toml)
    );
}

#[test]
fn format_from_extension_json() {
    assert_eq!(
        ConfigFormat::from_extension("json"),
        Some(ConfigFormat::Json)
    );
}

#[test]
fn format_from_extension_yaml() {
    assert_eq!(
        ConfigFormat::from_extension("yaml"),
        Some(ConfigFormat::Yaml)
    );
    assert_eq!(
        ConfigFormat::from_extension("yml"),
        Some(ConfigFormat::Yaml)
    );
    assert_eq!(
        ConfigFormat::from_extension("YML"),
        Some(ConfigFormat::Yaml)
    );
}

#[test]
fn format_from_extension_unknown() {
    assert_eq!(ConfigFormat::from_extension("xml"), None);
    assert_eq!(ConfigFormat::from_extension(""), None);
}

// -- ConfigFormat::from_path --

#[test]
fn format_from_path() {
    assert_eq!(
        ConfigFormat::from_path(Path::new("config.toml")),
        Some(ConfigFormat::Toml)
    );
    assert_eq!(
        ConfigFormat::from_path(Path::new("/etc/app/config.json")),
        Some(ConfigFormat::Json)
    );
    assert_eq!(
        ConfigFormat::from_path(Path::new("config.yaml")),
        Some(ConfigFormat::Yaml)
    );
    assert_eq!(ConfigFormat::from_path(Path::new("Makefile")), None);
}

// -- parse_config --

const TOML_CONTENT: &str = r#"
[model]
provider = "openai"
model = "gpt-4"
"#;

const JSON_CONTENT: &str = r#"{
    "model": {
        "provider": "openai",
        "model": "gpt-4"
    }
}"#;

const YAML_CONTENT: &str = r#"
model:
  provider: openai
  model: gpt-4
"#;

use synaptic_config::SynapticAgentConfig;

#[test]
fn parse_config_toml() {
    let config: SynapticAgentConfig = parse_config(TOML_CONTENT, ConfigFormat::Toml).unwrap();
    assert_eq!(config.model.provider, "openai");
    assert_eq!(config.model.model, "gpt-4");
}

#[test]
fn parse_config_json() {
    let config: SynapticAgentConfig = parse_config(JSON_CONTENT, ConfigFormat::Json).unwrap();
    assert_eq!(config.model.provider, "openai");
    assert_eq!(config.model.model, "gpt-4");
}

#[test]
fn parse_config_yaml() {
    let config: SynapticAgentConfig = parse_config(YAML_CONTENT, ConfigFormat::Yaml).unwrap();
    assert_eq!(config.model.provider, "openai");
    assert_eq!(config.model.model, "gpt-4");
}

#[test]
fn parse_config_invalid() {
    let result: Result<SynapticAgentConfig, _> = parse_config("not valid {{{", ConfigFormat::Toml);
    assert!(result.is_err());

    let result: Result<SynapticAgentConfig, _> = parse_config("not valid json", ConfigFormat::Json);
    assert!(result.is_err());

    let result: Result<SynapticAgentConfig, _> =
        parse_config(":\n  bad:\nyaml: [", ConfigFormat::Yaml);
    assert!(result.is_err());
}

/// Test with a generic T that uses #[serde(flatten)] — mirrors SynapseConfig's pattern.
#[test]
fn parse_config_generic_with_flatten() {
    #[derive(Debug, Deserialize)]
    struct Extended {
        #[serde(flatten)]
        base: SynapticAgentConfig,
        #[serde(default)]
        custom_field: Option<String>,
    }

    let json = r#"{
        "model": { "provider": "anthropic", "model": "claude-sonnet-4-20250514" },
        "custom_field": "hello"
    }"#;

    let ext: Extended = parse_config(json, ConfigFormat::Json).unwrap();
    assert_eq!(ext.base.model.provider, "anthropic");
    assert_eq!(ext.custom_field.as_deref(), Some("hello"));
}
