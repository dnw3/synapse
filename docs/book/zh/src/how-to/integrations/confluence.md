# Confluence 加载器

使用 Confluence REST API v2 将 Confluence Wiki 页面加载为 Synaptic 文档。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["confluence"] }
```

在 [id.atlassian.com/manage-profile/security/api-tokens](https://id.atlassian.com/manage-profile/security/api-tokens) 创建 API 令牌。

## 使用示例

```rust,ignore
use synaptic::confluence::{ConfluenceConfig, ConfluenceLoader};
use synaptic::core::Loader;

// 加载特定页面
let config = ConfluenceConfig::new(
    "yourcompany.atlassian.net",
    "you@example.com",
    "your-api-token",
)
.with_page_ids(vec!["12345678".to_string(), "87654321".to_string()]);
let loader = ConfluenceLoader::new(config);

// 加载整个空间的所有页面
let config = ConfluenceConfig::new(
    "yourcompany.atlassian.net",
    "you@example.com",
    "your-api-token",
)
.with_space_key("ENGDOCS");
let docs = ConfluenceLoader::new(config).load().await?;

for doc in &docs {
    println!("标题：{}", doc.metadata["title"]);
    println!("内容：{}", &doc.content[..200.min(doc.content.len())]);
}
```

## 元数据字段

每个文档包含以下元数据：

- `source` — `confluence:<page-id>`
- `title` — 页面标题
- `space_id` — 空间标识符（如可用）

## 注意事项

HTML 存储格式会被转换为纯文本。加载失败的页面会输出警告并跳过，不会中断整个加载操作。
