# 开发环境搭建

本页面介绍在本地构建、测试和运行 Synaptic 所需的一切。

## 前置条件

- **Rust 1.88 或更高版本** -- 通过 [rustup](https://rustup.rs/) 安装：
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  验证安装：
  ```bash
  rustc --version   # Should print 1.88.0 or later
  cargo --version
  ```

- **cargo** -- 随 Rust 工具链一起安装，无需单独安装。

## 克隆仓库

```bash
git clone https://github.com/<your-username>/synaptic.git
cd synaptic
```

## 构建

构建工作区中的所有 crate：

```bash
cargo build --workspace
```

## 测试

### 运行所有测试

```bash
cargo test --workspace
```

这将运行所有 17 个库 crate 的单元测试和集成测试。

### 测试单个 Crate

```bash
cargo test -p synaptic-tools
```

将 `synaptic-tools` 替换为工作区中的任何 crate 名称。

### 按名称运行特定测试

```bash
cargo test -p synaptic-core -- chunk
```

这只运行 `synaptic-core` crate 中名称包含 "chunk" 的测试。

## 运行示例

`examples/` 目录包含演示常见模式的可运行二进制文件：

```bash
cargo run -p react_basic
```

列出所有可用的示例目标：

```bash
ls examples/
```

## Lint 检查

运行 Clippy 捕获常见错误并强制执行地道的模式：

```bash
cargo clippy --workspace
```

提交变更前修复所有警告。

## 格式化

检查所有代码是否遵循标准 Rust 格式：

```bash
cargo fmt --all -- --check
```

如果检查失败，使用以下命令自动格式化：

```bash
cargo fmt --all
```

### Pre-commit Hook

仓库自带 pre-commit hook，会在每次提交前自动运行 `cargo fmt --check`。克隆后启用一次即可：

```bash
git config core.hooksPath .githooks
```

如果格式检查未通过，hook 会自动执行 `cargo fmt --all` — 你只需重新暂存变更并再次提交。

## 本地构建文档

### API 文档（rustdoc）

生成并在浏览器中打开完整的 API 参考：

```bash
cargo doc --workspace --open
```

### mdBook 站点

文档站点使用 [mdBook](https://rust-lang.github.io/mdBook/) 构建。安装 mdBook 并在本地提供英文文档：

```bash
cargo install mdbook
mdbook serve docs/book/en
```

这将启动一个本地服务器（通常在 `http://localhost:3000`），支持实时重载。编辑 `docs/book/en/src/` 下的任何 `.md` 文件，浏览器会自动更新。

不启动服务直接构建：

```bash
mdbook build docs/book/en
```

输出会写入 `docs/book/en/book/`。

## 编辑器设置

Synaptic 是一个标准的 Cargo 工作区。任何支持 rust-analyzer 的编辑器都能提供内联错误提示、自动补全和跨 crate 的跳转定义功能。推荐：

- **VS Code** 配合 rust-analyzer 扩展
- **IntelliJ IDEA** 配合 Rust 插件
- **Neovim** 通过 LSP 使用 rust-analyzer

## 环境变量

部分提供商适配器在运行时需要 API 密钥（不是在构建时）：

| 变量 | 使用者 |
|----------|---------|
| `OPENAI_API_KEY` | `OpenAiChatModel`, `OpenAiEmbeddings` |
| `ANTHROPIC_API_KEY` | `AnthropicChatModel` |
| `GOOGLE_API_KEY` | `GeminiChatModel` |

这些仅在运行调用真实提供商 API 的示例或测试时需要。测试套件使用 `ScriptedChatModel`、`FakeBackend` 和 `FakeEmbeddings` 进行离线测试，因此你可以在没有任何 API 密钥的情况下运行 `cargo test --workspace`。
