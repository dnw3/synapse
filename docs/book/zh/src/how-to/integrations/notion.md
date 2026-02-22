# Notion 加载器

使用 Notion API 将 Notion 页面内容加载为 Synaptic 文档。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

在 [notion.so/my-integrations](https://www.notion.so/my-integrations) 创建集成并获取内部集成令牌，然后将页面共享给该集成。

## 使用示例

```rust,ignore
use synaptic::loaders::NotionLoader;
use synaptic::core::Loader;

let loader = NotionLoader::new("secret_your_token", vec![
    "page-id-1".to_string(),
    "page-id-2".to_string(),
]);

let docs = loader.load().await?;
for doc in &docs {
    println!("页面标题：{}", doc.metadata["title"]);
    println!("内容：{}", &doc.content[..200.min(doc.content.len())]);
}
```

## 元数据字段

每个文档包含以下元数据：

- `source` — `notion:<page-id>`
- `title` — 从页面属性中提取的标题

## 支持的块类型

支持提取：段落、标题（H1/H2/H3）、无序列表、有序列表、引用、标注和代码块。其他块类型（图片、嵌入、数据库等）将被跳过。
