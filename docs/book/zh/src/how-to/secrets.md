# 密钥管理

`SecretRegistry` 管理敏感值（API 密钥、密码、令牌），确保它们不会在 AI 输出中泄露。

## SecretRegistry

注册密钥的名称和值。注册表可以在文本中遮蔽密钥值，并将其注入模板。

```rust,ignore
use synaptic::secrets::SecretRegistry;

let registry = SecretRegistry::new();
```

### 注册密钥

```rust,ignore
// 默认遮蔽格式：[REDACTED:name]
registry.register("api_key", "sk-abc123");

// 自定义遮蔽格式
registry.register_with_mask("db_password", "p@ssw0rd", "****");
```

### 遮蔽输出

将文本中所有已注册密钥值的出现替换为遮蔽字符。

```rust,ignore
let text = "The key is sk-abc123 and password is p@ssw0rd";
let masked = registry.mask_output(text);
assert_eq!(masked, "The key is [REDACTED:api_key] and password is ****");
```

### 注入密钥

使用 `{{secret:name}}` 语法将密钥值插入模板。

```rust,ignore
let template = "Connect to DB with password {{secret:db_password}}";
let resolved = registry.inject(template)?;
assert_eq!(resolved, "Connect to DB with password p@ssw0rd");
```

### 移除密钥

```rust,ignore
registry.remove("api_key");
```

## SecretMaskingMiddleware

该中间件自动集成到 Agent 生命周期中：

- **模型调用前**：将密钥注入系统提示词模板
- **模型调用后**：遮蔽 AI 响应中泄露的任何密钥

```rust,ignore
use synaptic::secrets::SecretMaskingMiddleware;

let registry = Arc::new(SecretRegistry::new());
registry.register("api_key", "sk-abc123");

let middleware = SecretMaskingMiddleware::new(registry);

let options = AgentOptions {
    middleware: vec![Arc::new(middleware)],
    ..Default::default()
};
```

这确保即使模型在响应中包含了密钥值，也会在响应到达用户之前自动替换为对应的遮蔽字符。
