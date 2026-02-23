# Configuration

`SynapticAgentConfig` provides multi-format configuration for agents, covering model settings, tool options, paths, and MCP servers. Supported formats: **TOML**, **JSON**, and **YAML**.

## Configuration File Examples

### TOML

```toml
[model]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"   # env var name (default)
max_tokens = 4096
temperature = 0.7

[agent]
system_prompt = "You are a helpful coding assistant."
max_turns = 50

[agent.tools]
filesystem = true
sandbox_root = "/tmp/sandbox"

[paths]
sessions_dir = ".sessions"
memory_file = "AGENTS.md"
skills_dir = ".skills"

[[mcp]]
name = "filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]

[[mcp]]
name = "web"
transport = "sse"
url = "http://localhost:8080/sse"
```

### JSON

```json
{
  "model": {
    "provider": "openai",
    "model": "gpt-4",
    "api_key_env": "OPENAI_API_KEY",
    "max_tokens": 4096,
    "temperature": 0.7
  },
  "agent": {
    "system_prompt": "You are a helpful coding assistant.",
    "max_turns": 50,
    "tools": { "filesystem": true }
  },
  "paths": {
    "sessions_dir": ".sessions",
    "memory_file": "AGENTS.md"
  }
}
```

### YAML

```yaml
model:
  provider: openai
  model: gpt-4
  api_key_env: OPENAI_API_KEY
  max_tokens: 4096
  temperature: 0.7

agent:
  system_prompt: "You are a helpful coding assistant."
  max_turns: 50
  tools:
    filesystem: true

paths:
  sessions_dir: ".sessions"
  memory_file: "AGENTS.md"
```

## Loading Configuration

`load()` searches for configuration files in this order:

1. Explicit path (if provided) — format auto-detected by extension
2. `./synaptic.{toml,json,yaml,yml}` in the current directory
3. `~/.synaptic/config.{toml,json,yaml,yml}` (global config)

```rust,ignore
use synaptic::config::SynapticAgentConfig;

// Auto-discover config file
let config = SynapticAgentConfig::load(None)?;

// Or specify a path (any supported format)
let config = SynapticAgentConfig::load(Some(Path::new("./my-agent.json")))?;
```

## Parsing from a String

```rust,ignore
use synaptic::config::{SynapticAgentConfig, ConfigFormat};

let yaml_str = r#"
model:
  provider: openai
  model: gpt-4
"#;
let config = SynapticAgentConfig::parse(yaml_str, ConfigFormat::Yaml)?;
```

## ConfigSource Trait

The `ConfigSource` trait abstracts where configuration comes from. Built-in implementations:

- **`FileConfigSource`** — loads from a local file (format auto-detected by extension)
- **`StringConfigSource`** — loads from an in-memory string (useful for tests or config-center payloads)

Future implementations can support remote config centers (Apollo, Nacos, etcd).

```rust,ignore
use synaptic::config::{SynapticAgentConfig, FileConfigSource, StringConfigSource, ConfigFormat};

// Load from a specific file
let source = FileConfigSource::new("./custom-config.yaml");
let config = SynapticAgentConfig::load_from(&source)?;

// Load from a string (e.g., fetched from a config center)
let source = StringConfigSource::new(json_string, ConfigFormat::Json);
let config = SynapticAgentConfig::load_from(&source)?;
```

### Generic Loading

The `discover_and_load<T>()` function works with any `DeserializeOwned` type, making it easy for downstream projects to reuse the discovery logic:

```rust,ignore
use synaptic::config::discover_and_load;

// Works with any Deserialize type — e.g., a product config that flattens SynapticAgentConfig
let config: MyProductConfig = discover_and_load(None)?;
```

## Resolving API Keys

The API key is read from the environment variable specified in `model.api_key_env`.

```rust,ignore
let api_key = config.resolve_api_key()?;
```

## Config Structs

### ModelConfig

| Field        | Type              | Default            |
|-------------|-------------------|--------------------|
| `provider`   | `String`          | required           |
| `model`      | `String`          | required           |
| `api_key_env`| `String`          | `"OPENAI_API_KEY"` |
| `base_url`   | `Option<String>`  | `None`             |
| `max_tokens` | `Option<u32>`     | `None`             |
| `temperature`| `Option<f64>`     | `None`             |

### AgentConfig

| Field          | Type              | Default |
|---------------|-------------------|---------|
| `system_prompt`| `Option<String>`  | `None`  |
| `max_turns`    | `Option<usize>`   | `None`  |
| `tools`        | `ToolsConfig`     | default |

### PathsConfig

| Field         | Type     | Default        |
|--------------|----------|----------------|
| `sessions_dir`| `String` | `".sessions"`  |
| `memory_file` | `String` | `"AGENTS.md"`  |
| `skills_dir`  | `String` | `".skills"`    |

### McpServerConfig

| Field       | Type                            | Required               |
|------------|----------------------------------|------------------------|
| `name`      | `String`                        | yes                    |
| `transport` | `String`                        | yes (`stdio`/`sse`/`http`) |
| `command`   | `Option<String>`                | stdio only             |
| `args`      | `Option<Vec<String>>`           | stdio only             |
| `url`       | `Option<String>`                | sse/http only          |
| `headers`   | `Option<HashMap<String, String>>`| sse/http only         |
