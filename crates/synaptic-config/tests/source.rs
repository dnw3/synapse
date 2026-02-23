use std::path::PathBuf;

use synaptic_config::{
    discover_and_load, ConfigFormat, FileConfigSource, StringConfigSource, SynapticAgentConfig,
};

fn temp_file(content: &str, ext: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "synaptic_source_test_{}.{ext}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, content).unwrap();
    path
}

const TOML_CONTENT: &str = r#"
[model]
provider = "openai"
model = "gpt-4"
"#;

const JSON_CONTENT: &str = r#"{
    "model": {
        "provider": "anthropic",
        "model": "claude-sonnet-4-20250514"
    }
}"#;

const YAML_CONTENT: &str = r#"
model:
  provider: gemini
  model: gemini-pro
"#;

// -- FileConfigSource --

#[test]
fn file_source_toml() {
    let path = temp_file(TOML_CONTENT, "toml");
    let source = FileConfigSource::new(&path);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "openai");
    std::fs::remove_file(&path).ok();
}

#[test]
fn file_source_json() {
    let path = temp_file(JSON_CONTENT, "json");
    let source = FileConfigSource::new(&path);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "anthropic");
    std::fs::remove_file(&path).ok();
}

#[test]
fn file_source_yaml() {
    let path = temp_file(YAML_CONTENT, "yaml");
    let source = FileConfigSource::new(&path);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "gemini");
    std::fs::remove_file(&path).ok();
}

#[test]
fn file_source_format_override() {
    // Write JSON content to a .txt file, then override format
    let path = temp_file(JSON_CONTENT, "txt");
    let source = FileConfigSource::new(&path).with_format(ConfigFormat::Json);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "anthropic");
    std::fs::remove_file(&path).ok();
}

#[test]
fn file_source_missing() {
    let source = FileConfigSource::new("/tmp/nonexistent_synaptic_config_abc123.toml");
    let result: Result<SynapticAgentConfig, _> = synaptic_config::load_from_source(&source);
    assert!(result.is_err());
}

#[test]
fn file_source_unknown_extension() {
    let path = temp_file(TOML_CONTENT, "cfg");
    let source = FileConfigSource::new(&path);
    let result: Result<SynapticAgentConfig, _> = synaptic_config::load_from_source(&source);
    assert!(result.is_err()); // cannot detect format
    std::fs::remove_file(&path).ok();
}

// -- StringConfigSource --

#[test]
fn string_source() {
    let source = StringConfigSource::new(JSON_CONTENT, ConfigFormat::Json);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "anthropic");
    assert_eq!(config.model.model, "claude-sonnet-4-20250514");
}

#[test]
fn string_source_yaml() {
    let source = StringConfigSource::new(YAML_CONTENT, ConfigFormat::Yaml);
    let config: SynapticAgentConfig = synaptic_config::load_from_source(&source).unwrap();
    assert_eq!(config.model.provider, "gemini");
}

// -- discover_and_load --

#[test]
fn discover_with_explicit_path_toml() {
    let path = temp_file(TOML_CONTENT, "toml");
    let config: SynapticAgentConfig = discover_and_load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "openai");
    std::fs::remove_file(&path).ok();
}

#[test]
fn discover_with_explicit_path_json() {
    let path = temp_file(JSON_CONTENT, "json");
    let config: SynapticAgentConfig = discover_and_load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "anthropic");
    std::fs::remove_file(&path).ok();
}

#[test]
fn discover_with_missing_path() {
    let result: Result<SynapticAgentConfig, _> =
        discover_and_load(Some(std::path::Path::new("/tmp/no_such_config_file.toml")));
    assert!(result.is_err());
}
