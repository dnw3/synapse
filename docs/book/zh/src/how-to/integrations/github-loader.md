# GitHub 加载器

通过 GitHub Contents API 从 GitHub 仓库加载源代码文件和文档。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

## 使用示例

```rust,ignore
use synaptic::loaders::GitHubLoader;
use synaptic::core::Loader;

// 加载单个文件
let loader = GitHubLoader::new("rust-lang", "rust", vec!["README.md".to_string()])
    .with_token("ghp_your_token")
    .with_branch("master");

// 递归加载目录中所有 .rs 文件
let loader = GitHubLoader::new("owner", "repo", vec!["src/".to_string()])
    .with_token("ghp_your_token")
    .with_extensions(vec![".rs".to_string(), ".md".to_string()]);

let docs = loader.load().await?;
for doc in &docs {
    println!("文件：{}", doc.id);
    println!("来源：{}", doc.metadata["source"]);
}
```

## 配置选项

| 方法 | 说明 |
|------|------|
| `with_token(token)` | GitHub 个人访问令牌（私有仓库或提高速率限制） |
| `with_branch(branch)` | 指定分支/标签/提交 SHA |
| `with_extensions(exts)` | 按扩展名过滤文件（如 `[".rs", ".md"]`），空值表示加载所有文件 |

## 元数据字段

每个文档包含以下元数据：

- `source` — `github:<owner>/<repo>/<path>`
- `sha` — 文件的 git blob SHA
- `branch` — 分支名（如已指定）
