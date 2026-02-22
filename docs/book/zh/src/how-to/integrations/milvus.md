# Milvus

[Milvus](https://milvus.io/) 是专为十亿规模近似最近邻搜索（ANNS）构建的向量数据库。`synaptic-milvus` crate 通过 Milvus REST API v2 实现 `VectorStore` trait。

## 安装

在 `Cargo.toml` 中添加 feature 标志：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["milvus"] }
```

使用 Docker 本地运行 Milvus：

```bash
docker run -d --name milvus-standalone \
  -p 19530:19530 -p 9091:9091 \
  milvusdb/milvus:latest standalone
```

## 使用示例

```rust,ignore
use synaptic::milvus::{MilvusConfig, MilvusVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MilvusConfig::new("http://localhost:19530", "my_collection", 1536);
    let store = MilvusVectorStore::new(config);

    // 创建集合（幂等 — 每次启动时调用均安全）
    store.initialize().await?;

    // 添加文档
    // store.add_documents(docs, &embeddings).await?;

    // 搜索
    // let results = store.similarity_search("查询文本", 5, &embeddings).await?;

    Ok(())
}
```

## Zilliz Cloud

对于 Zilliz Cloud（托管 Milvus），请添加 API 密钥：

```rust,ignore
let config = MilvusConfig::new("https://your-cluster.zillizcloud.com", "collection", 1536)
    .with_api_key("your-api-key");
```

## 配置参数

| 字段 | 类型 | 说明 |
|---|---|---|
| `endpoint` | `String` | Milvus 端点 URL（例如 `http://localhost:19530`） |
| `collection` | `String` | 集合名称 |
| `dim` | `usize` | 向量维度——必须与嵌入模型输出维度匹配 |
| `api_key` | `Option<String>` | Zilliz Cloud 身份验证 API 密钥 |
