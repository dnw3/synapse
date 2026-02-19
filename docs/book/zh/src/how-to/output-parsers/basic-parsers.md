# 基础解析器

Synaptic 提供了多种简单的 OutputParser，用于常见的转换操作。每个解析器都实现了 `Runnable`，因此可以单独使用或在管道中组合。

## StrOutputParser

从 `Message` 中提取文本内容。这是最常用的解析器 -- 它位于大多数链的末端，将模型的响应转换为纯 `String`。

**签名：** `Runnable<Message, String>`

```rust
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::{Message, RunnableConfig};

let parser = StrOutputParser;
let config = RunnableConfig::default();

let result = parser.invoke(Message::ai("Hello world"), &config).await?;
assert_eq!(result, "Hello world");
```

`StrOutputParser` 适用于任何 `Message` 变体 -- 系统、用户、AI 或工具消息都有可以提取的内容。

## JsonOutputParser

将 JSON 字符串解析为 `serde_json::Value`。当你需要处理任意 JSON 结构而不想定义特定的 Rust 类型时非常有用。

**签名：** `Runnable<String, serde_json::Value>`

```rust
use synaptic_parsers::JsonOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = JsonOutputParser;
let config = RunnableConfig::default();

let result = parser.invoke(
    r#"{"name": "Synaptic", "version": 1}"#.to_string(),
    &config,
).await?;

assert_eq!(result["name"], "Synaptic");
assert_eq!(result["version"], 1);
```

如果输入不是有效的 JSON，解析器返回 `Err(SynapticError::Parsing(...))`。

## ListOutputParser

使用可配置的分隔符将字符串拆分为 `Vec<String>`。当你要求 LLM 返回逗号分隔或换行分隔的列表时非常有用。

**签名：** `Runnable<String, Vec<String>>`

```rust
use synaptic_parsers::{ListOutputParser, ListSeparator};
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let config = RunnableConfig::default();

// Split on commas
let parser = ListOutputParser::comma();
let result = parser.invoke("apple, banana, cherry".to_string(), &config).await?;
assert_eq!(result, vec!["apple", "banana", "cherry"]);

// Split on newlines (default)
let parser = ListOutputParser::newline();
let result = parser.invoke("first\nsecond\nthird".to_string(), &config).await?;
assert_eq!(result, vec!["first", "second", "third"]);

// Custom separator
let parser = ListOutputParser::new(ListSeparator::Custom("|".to_string()));
let result = parser.invoke("a | b | c".to_string(), &config).await?;
assert_eq!(result, vec!["a", "b", "c"]);
```

每个项目的前后空白会被去除。去除空白后为空的项目会被过滤掉。

## BooleanOutputParser

将 yes/no、true/false、y/n 和 1/0 类型的响应解析为 `bool`。不区分大小写，并去除空白。

**签名：** `Runnable<String, bool>`

```rust
use synaptic_parsers::BooleanOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = BooleanOutputParser;
let config = RunnableConfig::default();

assert_eq!(parser.invoke("Yes".to_string(), &config).await?, true);
assert_eq!(parser.invoke("false".to_string(), &config).await?, false);
assert_eq!(parser.invoke("1".to_string(), &config).await?, true);
assert_eq!(parser.invoke("N".to_string(), &config).await?, false);
```

无法识别的值返回 `Err(SynapticError::Parsing(...))`。

## XmlOutputParser

将 XML 格式的 LLM 输出解析为 `XmlElement` 树。支持嵌套元素、属性和文本内容，无需完整的 XML 库。

**签名：** `Runnable<String, XmlElement>`

```rust
use synaptic_parsers::{XmlOutputParser, XmlElement};
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let config = RunnableConfig::default();

// Parse with a root tag filter
let parser = XmlOutputParser::with_root_tag("answer");
let result = parser.invoke(
    "Here is my answer: <answer><item>hello</item></answer>".to_string(),
    &config,
).await?;

assert_eq!(result.tag, "answer");
assert_eq!(result.children[0].tag, "item");
assert_eq!(result.children[0].text, Some("hello".to_string()));
```

使用 `XmlOutputParser::new()` 将整个输入解析为 XML，或使用 `with_root_tag("tag")` 从特定根标签中提取内容。

## MarkdownListOutputParser

解析 Markdown 格式的无序列表（`- item` 或 `* item`）为 `Vec<String>`。不以列表标记开头的行会被忽略。

**签名：** `Runnable<String, Vec<String>>`

```rust
use synaptic_parsers::MarkdownListOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = MarkdownListOutputParser;
let config = RunnableConfig::default();

let result = parser.invoke(
    "Here are the items:\n- Apple\n- Banana\n* Cherry\nNot a list item".to_string(),
    &config,
).await?;

assert_eq!(result, vec!["Apple", "Banana", "Cherry"]);
```

## NumberedListOutputParser

将有序列表（`1. item`、`2. item`）解析为 `Vec<String>`。数字前缀会被去除；只有匹配 `N. text` 模式的行才会被包含。

**签名：** `Runnable<String, Vec<String>>`

```rust
use synaptic_parsers::NumberedListOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::RunnableConfig;

let parser = NumberedListOutputParser;
let config = RunnableConfig::default();

let result = parser.invoke(
    "Top 3 languages:\n1. Rust\n2. Python\n3. TypeScript".to_string(),
    &config,
).await?;

assert_eq!(result, vec!["Rust", "Python", "TypeScript"]);
```

## FormatInstructions

所有解析器都实现了 `FormatInstructions` trait。你可以在提示词中包含这些指令来引导模型：

```rust
use synaptic_parsers::{JsonOutputParser, ListOutputParser, FormatInstructions};

let json_parser = JsonOutputParser;
println!("{}", json_parser.get_format_instructions());
// "Your response should be a valid JSON object."

let list_parser = ListOutputParser::comma();
println!("{}", list_parser.get_format_instructions());
// "Your response should be a list of items separated by commas."
```

## 管道示例

一个典型的链将提示词模板通过模型传入解析器：

```rust
use std::collections::HashMap;
use serde_json::json;
use synaptic_core::{ChatResponse, Message, RunnableConfig};
use synaptic_models::ScriptedChatModel;
use synaptic_prompts::{ChatPromptTemplate, MessageTemplate};
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;

let model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("The answer is 42."),
        usage: None,
    },
]);

let template = ChatPromptTemplate::from_messages(vec![
    MessageTemplate::system("You are a helpful assistant."),
    MessageTemplate::human("{{ question }}"),
]);

// template -> model -> parser
let chain = template.boxed() | model.boxed() | StrOutputParser.boxed();

let config = RunnableConfig::default();
let values: HashMap<String, serde_json::Value> = HashMap::from([
    ("question".to_string(), json!("What is the meaning of life?")),
]);

let result: String = chain.invoke(values, &config).await?;
assert_eq!(result, "The answer is 42.");
```
