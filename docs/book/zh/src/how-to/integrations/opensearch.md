# OpenSearch

[OpenSearch](https://opensearch.org/) 是一款开源搜索和分析引擎，内置 k-NN（k 近邻）插件支持近似向量搜索。`synaptic-opensearch` crate 使用 OpenSearch 基于 HNSW 的 k-NN 索引实现 `VectorStore` trait。

## 安装

在 `Cargo.toml` 中添加 feature 标志：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["opensearch"] }
```

使用 Docker 本地运行 OpenSearch：

```bash
docker run -d --name opensearch \
  -p 9200:9200 -p 9600:9600 \
  -e "discovery.type=single-node" \
  -e "plugins.security.disabled=true" \
  opensearchproject/opensearch:latest
```

## 使用示例

```rust,ignore
use synaptic::opensearch::{OpenSearchConfig, OpenSearchVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OpenSearchConfig::new("http://localhost:9200", "my_index", 1536)
        .with_credentials("admin", "admin");
    let store = OpenSearchVectorStore::new(config);

    // 创建带有 k-NN 映射的索引（幂等）
    store.initialize().await?;

    // 添加文档
    // store.add_documents(docs, &embeddings).await?;

    // 搜索
    // let results = store.similarity_search("查询文本", 5, &embeddings).await?;

    Ok(())
}
```

## Amazon OpenSearch Service

对于 Amazon OpenSearch Service，请将端点设置为 AWS 预配置的域名：

```rust,ignore
let config = OpenSearchConfig::new(
    "https://my-domain.us-east-1.es.amazonaws.com",
    "my_index",
    1536,
);
```

## 配置参数

| 字段 | 类型 | 说明 |
|---|---|---|
| `endpoint` | `String` | OpenSearch 端点 URL（例如 `http://localhost:9200`） |
| `index` | `String` | 索引名称 |
| `dim` | `usize` | 向量维度——必须与嵌入模型输出维度匹配 |
| `username` | `Option<String>` | HTTP Basic Auth 用户名 |
| `password` | `Option<String>` | HTTP Basic Auth 密码 |
