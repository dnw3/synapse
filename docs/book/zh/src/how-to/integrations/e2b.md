# E2B 沙箱

[E2B](https://e2b.dev/) 提供基于云的代码执行沙箱。`E2BSandboxTool` 允许 LLM 智能体安全地执行代码——每次调用创建一个隔离沙箱，运行代码，然后销毁环境。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["e2b"] }
```

在 [e2b.dev](https://e2b.dev/) 获取 API 密钥。

## 使用示例

```rust,ignore
use synaptic::e2b::{E2BConfig, E2BSandboxTool};
use synaptic::core::Tool;
use serde_json::json;

let config = E2BConfig::new("your-api-key")
    .with_template("python")
    .with_timeout(30);
let tool = E2BSandboxTool::new(config);

let result = tool.call(json!({
    "code": "print(sum(range(1, 101)))",
    "language": "python"
})).await?;
// {"stdout": "5050\n", "stderr": "", "exit_code": 0}
```

## 配置参数

| 字段 | 默认值 | 说明 |
|---|---|---|
| `template` | `"base"` | 沙箱模板（`"base"`、`"python"`、`"nodejs"`） |
| `timeout_secs` | `30` | 执行超时时间（秒） |

## 支持的编程语言

| 语言 | 参数值 |
|---|---|
| Python | `"python"` |
| JavaScript | `"javascript"` |
| Bash | `"bash"` |

## 注意事项

- 每次工具调用都会创建一个全新沙箱，执行结束后立即销毁，保证隔离性。
- 即使代码执行失败，沙箱也会被销毁。
- 沙箱内的网络访问权限取决于 E2B 模板配置。
