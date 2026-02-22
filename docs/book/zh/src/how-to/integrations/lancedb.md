# LanceDB

[LanceDB](https://lancedb.github.io/lancedb/) 是一个无服务器的嵌入式向量数据库——在进程内运行，无需独立服务器。数据以 [Lance](https://github.com/lancedb/lance) 列式格式存储在本地磁盘或云对象存储（S3、GCS、Azure Blob）中。

## 安装

在 `Cargo.toml` 中添加 feature 标志：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["lancedb"] }
```

无需 Docker 容器或外部服务。

## 依赖说明

`lancedb` crate（>= 0.20）具有要求 Rust >= 1.91 的传递依赖项。当前 `synaptic-lancedb` crate 提供一个纯 Rust 内存后端，实现完整的 `VectorStore` 接口，使您的应用程序能够在 MSRV 1.88 下编译和测试。待工具链需求对齐后，实现将升级为使用原生 Lance 磁盘存储。

## 使用示例

```rust,ignore
use synaptic::lancedb::{LanceDbConfig, LanceDbVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 基于本地文件的存储
    let config = LanceDbConfig::new("/var/lib/myapp/vectors", "documents", 1536);
    let store = LanceDbVectorStore::new(config).await?;

    // 添加文档
    // store.add_documents(docs, &embeddings).await?;

    // 搜索
    // let results = store.similarity_search("查询文本", 5, &embeddings).await?;

    Ok(())
}
```

## 云端存储

当原生 lancedb 后端可用时，只需使用 S3 URI 即可支持 S3 存储：

```rust,ignore
let config = LanceDbConfig::new("s3://my-bucket/vectors", "documents", 1536);
let store = LanceDbVectorStore::new(config).await?;
```

## 配置参数

| 字段 | 类型 | 说明 |
|---|---|---|
| `uri` | `String` | 存储路径——本地（`/data/mydb`）或云端（`s3://bucket/path`） |
| `table_name` | `String` | 数据库中的表名 |
| `dim` | `usize` | 向量维度——必须与嵌入模型输出维度匹配 |

## 优势

- **无需服务器** — 完全在进程内运行
- **版本化** — Lance 格式支持时光旅行查询
- **云原生** — 支持 S3/GCS/Azure Blob 后端存储，无需中间服务
- **高吞吐量** — 列式格式针对扫描密集型向量工作负载优化
