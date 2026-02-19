# 消息

消息是 Synaptic 中通信的基本单元。与聊天模型的每次交互都表示为一系列 `Message` 值，每次响应也以 `Message` 形式返回。

`Message` 枚举定义在 `synaptic_core` 中，采用标签联合体（tagged union）设计，包含六个变体：`System`、`Human`、`AI`、`Tool`、`Chat` 和 `Remove`。消息通过工厂方法创建，而不是直接构造结构体字面量。

## 快速示例

```rust
use synaptic_core::{ChatRequest, Message};

let messages = vec![
    Message::system("You are a helpful assistant."),
    Message::human("What is Rust?"),
];

let request = ChatRequest::new(messages);
```

## 指南

- [消息类型](types.md) -- 所有消息变体、工厂方法和访问器方法
- [过滤与裁剪消息](filter-trim.md) -- 按类型/名称/ID 筛选消息，以及按 token 预算裁剪
- [合并连续消息](merge-runs.md) -- 将相同角色的连续消息合并为一条
