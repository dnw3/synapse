# 快速开始

本指南将带你用不到 30 行代码编写你的第一个 Synaptic 程序。我们使用 `ScriptedChatModel`（一个测试替身），无需 API 密钥即可运行。

## 第 1 步：创建项目

```bash
cargo new synaptic-quickstart
cd synaptic-quickstart
```

## 第 2 步：添加依赖

编辑 `Cargo.toml`：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["models"] }
tokio = { version = "1.41", features = ["macros", "rt-multi-thread"] }
```

## 第 3 步：编写代码

将 `src/main.rs` 替换为以下内容：

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::ScriptedChatModel;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // 创建一个脚本化模型，预先定义好返回的响应
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("你好！我是 Synaptic 助手。"),
            usage: None,
        },
    ]);

    // 构建聊天请求：系统提示 + 用户消息
    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant."),
        Message::human("你好！"),
    ]);

    // 发送请求并获取响应
    let response = model.chat(request).await?;
    println!("{}", response.message.content());

    Ok(())
}
```

## 第 4 步：运行

```bash
cargo run
```

你应该会看到输出：

```
你好！我是 Synaptic 助手。
```

## 代码解析

让我们逐步解析上面的代码：

1. **`ScriptedChatModel`** -- 这是一个测试替身，按顺序返回预设的响应。在实际应用中，你会使用 `OpenAiChatModel`、`AnthropicChatModel` 等真实模型适配器。

2. **`Message` 工厂方法** -- Synaptic 使用枚举变体表示不同类型的消息：
   - `Message::system(...)` -- 系统提示，设定 AI 的行为
   - `Message::human(...)` -- 用户输入
   - `Message::ai(...)` -- AI 的回复
   - `Message::tool(...)` -- 工具调用的结果

3. **`ChatRequest::new(messages)`** -- 将消息列表包装为请求。可以链式调用 `.with_tools()` 和 `.with_tool_choice()` 添加工具支持。

4. **`model.chat(request).await?`** -- 异步发送请求。所有 Synaptic 的 trait 方法都是异步的，使用 `await` 等待结果，`?` 传播错误。

## 使用真实模型

要连接 OpenAI 等真实 LLM 提供商，只需替换模型实例：

```rust
use synaptic::models::OpenAiChatModel;

let model = OpenAiChatModel::new("gpt-4o");
```

确保已设置 `OPENAI_API_KEY` 环境变量。详见[安装](installation.md)中的环境变量说明。

## 下一步

- [构建一个简单的 LLM 应用](tutorials/simple-llm-app.md) -- 使用 Prompt Template 和 Output Parser 构建完整的链式调用
- [构建 ReAct Agent](tutorials/react-agent.md) -- 让 AI 调用工具并自主推理
- [架构概览](architecture-overview.md) -- 深入了解 Synaptic 的 crate 架构
