use std::path::PathBuf;
use synaptic_config::{ConfigFormat, SynapticAgentConfig};

fn temp_file(content: &str, ext: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "synaptic_config_test_{}.{ext}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, content).unwrap();
    path
}

fn temp_toml(content: &str) -> PathBuf {
    temp_file(content, "toml")
}

#[test]
fn load_valid_toml() {
    let path = temp_toml(
        r#"
[model]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"

[agent]
system_prompt = "You are helpful"
max_turns = 10
"#,
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "openai");
    assert_eq!(config.model.model, "gpt-4");
    assert_eq!(
        config.agent.system_prompt.as_deref(),
        Some("You are helpful")
    );
    assert_eq!(config.agent.max_turns, Some(10));

    std::fs::remove_file(&path).ok();
}

#[test]
fn missing_required_field() {
    let path = temp_toml(
        r#"
[model]
provider = "openai"
"#,
    );

    let result = SynapticAgentConfig::load(Some(&path));
    assert!(result.is_err()); // model.model is required

    std::fs::remove_file(&path).ok();
}

#[test]
fn resolve_api_key_from_env() {
    let path = temp_toml(
        r#"
[model]
provider = "openai"
model = "gpt-4"
api_key_env = "SYNAPTIC_TEST_KEY_12345"
"#,
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();

    // Key not set -> error
    let result = config.resolve_api_key();
    assert!(result.is_err());

    // Set the key
    std::env::set_var("SYNAPTIC_TEST_KEY_12345", "test-key-value");
    let key = config.resolve_api_key().unwrap();
    assert_eq!(key, "test-key-value");
    std::env::remove_var("SYNAPTIC_TEST_KEY_12345");

    std::fs::remove_file(&path).ok();
}

#[test]
fn default_paths() {
    let path = temp_toml(
        r#"
[model]
provider = "openai"
model = "gpt-4"
"#,
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    assert_eq!(config.paths.sessions_dir, ".sessions");
    assert_eq!(config.paths.memory_file, "AGENTS.md");
    assert_eq!(config.paths.skills_dir, ".skills");

    std::fs::remove_file(&path).ok();
}

#[test]
fn mcp_config_parsing() {
    let path = temp_toml(
        r#"
[model]
provider = "openai"
model = "gpt-4"

[[mcp]]
name = "filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@anthropic/mcp-server-filesystem"]

[[mcp]]
name = "web"
transport = "sse"
url = "http://localhost:8080/sse"
"#,
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    let mcp = config.mcp.unwrap();
    assert_eq!(mcp.len(), 2);
    assert_eq!(mcp[0].name, "filesystem");
    assert_eq!(mcp[0].transport, "stdio");
    assert_eq!(mcp[0].command.as_deref(), Some("npx"));
    assert_eq!(mcp[1].name, "web");
    assert_eq!(mcp[1].url.as_deref(), Some("http://localhost:8080/sse"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn unknown_provider_ok() {
    let path = temp_toml(
        r#"
[model]
provider = "my-custom-provider"
model = "custom-model"
base_url = "https://custom.api.com/v1"
"#,
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "my-custom-provider");
    assert_eq!(
        config.model.base_url.as_deref(),
        Some("https://custom.api.com/v1")
    );

    std::fs::remove_file(&path).ok();
}

// -- Multi-format loading via SynapticAgentConfig::load --

#[test]
fn load_json_file() {
    let path = temp_file(
        r#"{
    "model": {
        "provider": "anthropic",
        "model": "claude-sonnet-4-20250514"
    },
    "agent": {
        "max_turns": 20
    }
}"#,
        "json",
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "anthropic");
    assert_eq!(config.agent.max_turns, Some(20));

    std::fs::remove_file(&path).ok();
}

#[test]
fn load_yaml_file() {
    let path = temp_file(
        r#"
model:
  provider: gemini
  model: gemini-pro
  api_key_env: GEMINI_API_KEY
agent:
  system_prompt: "Hello from YAML"
"#,
        "yaml",
    );

    let config = SynapticAgentConfig::load(Some(&path)).unwrap();
    assert_eq!(config.model.provider, "gemini");
    assert_eq!(
        config.agent.system_prompt.as_deref(),
        Some("Hello from YAML")
    );

    std::fs::remove_file(&path).ok();
}

// -- SynapticAgentConfig::parse --

#[test]
fn load_from_string() {
    let config = SynapticAgentConfig::parse(
        r#"{ "model": { "provider": "openai", "model": "gpt-4o" } }"#,
        ConfigFormat::Json,
    )
    .unwrap();
    assert_eq!(config.model.model, "gpt-4o");
}
