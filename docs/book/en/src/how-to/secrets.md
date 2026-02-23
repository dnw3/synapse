# Secret Management

The `SecretRegistry` manages sensitive values (API keys, passwords, tokens) so they are never leaked in AI outputs.

## SecretRegistry

Register secrets with a name and value. The registry can mask occurrences of secret values in text and inject them into templates.

```rust,ignore
use synaptic::secrets::SecretRegistry;

let registry = SecretRegistry::new();
```

### Registering Secrets

```rust,ignore
// Default mask: [REDACTED:name]
registry.register("api_key", "sk-abc123");

// Custom mask
registry.register_with_mask("db_password", "p@ssw0rd", "****");
```

### Masking Output

Replace all occurrences of registered secret values in text with their masks.

```rust,ignore
let text = "The key is sk-abc123 and password is p@ssw0rd";
let masked = registry.mask_output(text);
assert_eq!(masked, "The key is [REDACTED:api_key] and password is ****");
```

### Injecting Secrets

Insert secret values into templates using `{{secret:name}}` syntax.

```rust,ignore
let template = "Connect to DB with password {{secret:db_password}}";
let resolved = registry.inject(template)?;
assert_eq!(resolved, "Connect to DB with password p@ssw0rd");
```

### Removing Secrets

```rust,ignore
registry.remove("api_key");
```

## SecretMaskingMiddleware

The middleware automatically integrates with the agent lifecycle:

- **Before model calls**: injects secrets into the system prompt template
- **After model calls**: masks any leaked secrets in the AI response

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

This ensures that even if the model includes a secret value in its response, it is automatically replaced with the corresponding mask before the response reaches the user.
