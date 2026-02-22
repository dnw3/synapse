# arXiv 加载器

将 arXiv 学术论文加载为 Synaptic 文档。返回论文摘要及标题、作者、发表日期等元数据。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

无需 API 密钥，arXiv 提供免费的公开 API。

## 使用示例

```rust,ignore
use synaptic::loaders::ArxivLoader;
use synaptic::core::Loader;

let loader = ArxivLoader::new("大语言模型 Rust")
    .with_max_results(5);

let docs = loader.load().await?;
for doc in &docs {
    println!("标题：{}", doc.metadata["title"]);
    println!("作者：{}", doc.metadata["authors"]);
    println!("发表时间：{}", doc.metadata["published"]);
    println!("摘要：{}", &doc.content[..200.min(doc.content.len())]);
}
```

## 元数据字段

每个文档包含以下元数据：

- `source` — `arxiv:<arxiv-id>`
- `url` — `https://arxiv.org/abs/<arxiv-id>`
- `title` — 论文标题
- `authors` — 逗号分隔的作者列表
- `published` — ISO 8601 格式的发表日期

## 注意事项

结果按提交日期降序排列（最新在前）。`doc.content` 字段包含摘要文本。arXiv API 有速率限制，批量请求时建议在请求之间添加延迟。
