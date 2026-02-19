# 贡献指南

感谢你对 Synaptic 项目的贡献兴趣。本指南介绍提交变更的工作流程和规范。

## 开始之前

1. 在 GitHub 上 **Fork** 本仓库。
2. 将你的 fork **克隆**到本地：
   ```bash
   git clone https://github.com/<your-username>/synaptic.git
   cd synaptic
   ```
3. 为你的变更**创建分支**：
   ```bash
   git checkout -b feature/my-change
   ```

## 开发工作流

在提交 pull request 之前，确保所有检查在本地通过。

### 运行测试

```bash
cargo test --workspace
```

所有测试必须通过。如果你正在添加新功能，请在对应 crate 的 `tests/` 目录中添加测试。

### 运行 Clippy

```bash
cargo clippy --workspace
```

修复所有警告。Clippy 强制执行地道的 Rust 模式并捕获常见错误。

### 检查格式

```bash
cargo fmt --all -- --check
```

如果检查失败，运行 `cargo fmt --all` 自动格式化，然后提交结果。

### 构建工作区

```bash
cargo build --workspace
```

确保所有内容编译通过。

## 提交 Pull Request

1. 将你的分支推送到你的 fork。
2. 针对 `main` 分支打开一个 pull request。
3. 提供清晰的描述，说明你的变更做了什么以及为什么。
4. 引用相关的 issue。

## 规范

### 代码

- 遵循代码库中的现有模式。每个 crate 都有一致的结构，`src/` 放实现，`tests/` 放集成测试。
- 所有 trait 通过 `#[async_trait]` 实现异步。测试使用 `#[tokio::test]`。
- 共享注册表使用 `Arc<RwLock<_>>`，回调和记忆使用 `Arc<tokio::sync::Mutex<_>>`。
- 核心类型优先使用工厂方法而非结构体字面量（例如 `Message::human()`、`ChatRequest::new()`）。

### 文档

- 添加新功能或变更公共 API 时，更新 `docs/book/en/src/` 中对应的文档页面。
- 操作指南放在 `how-to/`，概念说明放在 `concepts/`，分步教程放在 `tutorials/`。
- 如果你的变更影响了项目概览，请更新仓库根目录的 README。

### 测试

- 每个 crate 都有 `tests/` 目录，包含独立文件中的集成测试。
- 使用 `ScriptedChatModel` 或 `FakeBackend` 测试模型交互，无需真实 API 调用。
- 使用 `FakeEmbeddings` 测试依赖 embedding 的功能。

### 提交消息

- 编写清晰、简洁的提交消息，解释变更背后的"为什么"。
- 适当使用常规前缀：`feat:`、`fix:`、`docs:`、`refactor:`、`test:`。

## 项目结构

工作区包含 `crates/` 中的 17 个库 crate 和 `examples/` 中的示例二进制文件。详细的 crate 层次和依赖图请参阅[架构概览](architecture-overview.md)。

## 问题

如果你对某个方案不确定，请在编写代码前先开一个 issue 进行讨论。这有助于避免不必要的工作，并确保变更与项目方向一致。
