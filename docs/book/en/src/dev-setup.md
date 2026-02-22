# Development Setup

This page covers everything you need to build, test, and run Synaptic locally.

## Prerequisites

- **Rust 1.88 or later** -- Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  Verify with:
  ```bash
  rustc --version   # Should print 1.88.0 or later
  cargo --version
  ```

- **cargo** -- Included with the Rust toolchain. No separate install needed.

## Clone the Repository

```bash
git clone https://github.com/<your-username>/synaptic.git
cd synaptic
```

## Build

Build every crate in the workspace:

```bash
cargo build --workspace
```

## Test

### Run All Tests

```bash
cargo test --workspace
```

This runs unit tests and integration tests across all 17 library crates.

### Test a Single Crate

```bash
cargo test -p synaptic-tools
```

Replace `synaptic-tools` with any crate name from the workspace.

### Run a Specific Test by Name

```bash
cargo test -p synaptic-core -- chunk
```

This runs only tests whose names contain "chunk" within the `synaptic-core` crate.

## Run Examples

The `examples/` directory contains runnable binaries that demonstrate common patterns:

```bash
cargo run -p react_basic
```

List all available example targets with:

```bash
ls examples/
```

## Lint

Run Clippy to catch common mistakes and enforce idiomatic patterns:

```bash
cargo clippy --workspace
```

Fix any warnings before submitting changes.

## Format

Check that all code follows the standard Rust formatting:

```bash
cargo fmt --all -- --check
```

If this fails, auto-format with:

```bash
cargo fmt --all
```

### Pre-commit Hook

The repository ships a pre-commit hook that runs `cargo fmt --check` automatically before each commit. Enable it once after cloning:

```bash
git config core.hooksPath .githooks
```

If formatting fails the hook will run `cargo fmt --all` for you — just re-stage the changes and commit again.

## Build Documentation Locally

### API Docs (rustdoc)

Generate and open the full API reference in your browser:

```bash
cargo doc --workspace --open
```

### mdBook Site

The documentation site is built with [mdBook](https://rust-lang.github.io/mdBook/). Install it and serve the English docs locally:

```bash
cargo install mdbook
mdbook serve docs/book/en
```

This starts a local server (typically at `http://localhost:3000`) with live reload. Edit any `.md` file under `docs/book/en/src/` and the browser will update automatically.

To build the book without serving:

```bash
mdbook build docs/book/en
```

The output is written to `docs/book/en/book/`.

## Editor Setup

Synaptic is a standard Cargo workspace. Any editor with rust-analyzer support will provide inline errors, completions, and go-to-definition across all crates. Recommended:

- **VS Code** with the rust-analyzer extension
- **IntelliJ IDEA** with the Rust plugin
- **Neovim** with rust-analyzer via LSP

## Environment Variables

Some provider adapters require API keys at runtime (not at build time):

| Variable | Used by |
|----------|---------|
| `OPENAI_API_KEY` | `OpenAiChatModel`, `OpenAiEmbeddings` |
| `ANTHROPIC_API_KEY` | `AnthropicChatModel` |
| `GOOGLE_API_KEY` | `GeminiChatModel` |

These are only needed when running examples or tests that hit real provider APIs. The test suite uses `ScriptedChatModel`, `FakeBackend`, and `FakeEmbeddings` for offline testing, so you can run `cargo test --workspace` without any API keys.
