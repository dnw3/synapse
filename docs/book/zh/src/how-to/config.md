# 配置

`SynapticAgentConfig` 提供多格式 Agent 配置，涵盖模型设置、工具选项、路径和 MCP 服务器。支持格式：**TOML**、**JSON** 和 **YAML**。

## 配置文件示例

### TOML

```toml
[model]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"   # 环境变量名（默认值）
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

## 加载配置

`load()` 按以下顺序搜索配置文件：

1. 显式路径（如果提供）— 根据扩展名自动检测格式
2. 当前目录下的 `./synaptic.{toml,json,yaml,yml}`
3. `~/.synaptic/config.{toml,json,yaml,yml}`（全局配置）

```rust,ignore
use synaptic::config::SynapticAgentConfig;

// 自动发现配置文件
let config = SynapticAgentConfig::load(None)?;

// 或指定路径（支持任意格式）
let config = SynapticAgentConfig::load(Some(Path::new("./my-agent.json")))?;
```

## 从字符串解析

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

`ConfigSource` trait 抽象了配置的来源。内置实现：

- **`FileConfigSource`** — 从本地文件加载（根据扩展名自动检测格式）
- **`StringConfigSource`** — 从内存字符串加载（适用于测试或配置中心返回的原始内容）

未来可以为远程配置中心（Apollo、Nacos、etcd）提供实现。

```rust,ignore
use synaptic::config::{SynapticAgentConfig, FileConfigSource, StringConfigSource, ConfigFormat};

// 从指定文件加载
let source = FileConfigSource::new("./custom-config.yaml");
let config = SynapticAgentConfig::load_from(&source)?;

// 从字符串加载（例如从配置中心获取）
let source = StringConfigSource::new(json_string, ConfigFormat::Json);
let config = SynapticAgentConfig::load_from(&source)?;
```

### 泛型加载

`discover_and_load<T>()` 函数适用于任何 `DeserializeOwned` 类型，下游项目可以复用发现逻辑：

```rust,ignore
use synaptic::config::discover_and_load;

// 适用于任何 Deserialize 类型 — 例如通过 flatten 嵌入 SynapticAgentConfig 的产品配置
let config: MyProductConfig = discover_and_load(None)?;
```

## 解析 API 密钥

API 密钥从 `model.api_key_env` 指定的环境变量中读取。

```rust,ignore
let api_key = config.resolve_api_key()?;
```

## 配置结构体

### ModelConfig

| 字段          | 类型              | 默认值             |
|--------------|-------------------|--------------------|
| `provider`   | `String`          | 必填               |
| `model`      | `String`          | 必填               |
| `api_key_env`| `String`          | `"OPENAI_API_KEY"` |
| `base_url`   | `Option<String>`  | `None`             |
| `max_tokens` | `Option<u32>`     | `None`             |
| `temperature`| `Option<f64>`     | `None`             |

### AgentConfig

| 字段            | 类型              | 默认值  |
|----------------|-------------------|---------|
| `system_prompt`| `Option<String>`  | `None`  |
| `max_turns`    | `Option<usize>`   | `None`  |
| `tools`        | `ToolsConfig`     | 默认值  |

### PathsConfig

| 字段          | 类型     | 默认值         |
|--------------|----------|----------------|
| `sessions_dir`| `String` | `".sessions"`  |
| `memory_file` | `String` | `"AGENTS.md"`  |
| `skills_dir`  | `String` | `".skills"`    |

### McpServerConfig

| 字段        | 类型                             | 是否必填                |
|------------|----------------------------------|------------------------|
| `name`     | `String`                         | 是                     |
| `transport`| `String`                         | 是（`stdio`/`sse`/`http`）|
| `command`  | `Option<String>`                 | 仅 stdio               |
| `args`     | `Option<Vec<String>>`            | 仅 stdio               |
| `url`      | `Option<String>`                 | 仅 sse/http            |
| `headers`  | `Option<HashMap<String, String>>`| 仅 sse/http            |
