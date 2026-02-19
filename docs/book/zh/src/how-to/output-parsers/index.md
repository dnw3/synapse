# OutputParser

OutputParser 将原始 LLM 输出转换为结构化数据。Synaptic 中的每个解析器都实现了 `Runnable` trait，因此它们可以使用 LCEL 管道运算符（`|`）自然地与提示词模板、聊天模型和其他 Runnable 组合。

## 可用的解析器

| 解析器 | 输入 | 输出 | 描述 |
|--------|-------|--------|-------------|
| `StrOutputParser` | `Message` | `String` | 从消息中提取文本内容 |
| `JsonOutputParser` | `String` | `serde_json::Value` | 将字符串解析为 JSON |
| `StructuredOutputParser<T>` | `String` | `T` | 将 JSON 反序列化为类型化的结构体 |
| `ListOutputParser` | `String` | `Vec<String>` | 按可配置的分隔符拆分 |
| `EnumOutputParser` | `String` | `String` | 验证输出是否在允许值列表中 |
| `BooleanOutputParser` | `String` | `bool` | 解析 yes/no/true/false 字符串 |
| `MarkdownListOutputParser` | `String` | `Vec<String>` | 解析 Markdown 无序列表 |
| `NumberedListOutputParser` | `String` | `Vec<String>` | 解析有序列表 |
| `XmlOutputParser` | `String` | `XmlElement` | 将 XML 解析为树结构 |

所有解析器还实现了 `FormatInstructions` trait，该 trait 提供 `get_format_instructions()` 方法。你可以在提示词中包含这些指令，以引导 LLM 生成预期格式的输出。

## 快速示例

```rust
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::{Message, RunnableConfig};

let parser = StrOutputParser;
let config = RunnableConfig::default();
let result = parser.invoke(Message::ai("Hello world"), &config).await?;
assert_eq!(result, "Hello world");
```

## 子页面

- [基础解析器](basic-parsers.md) -- StrOutputParser、JsonOutputParser、ListOutputParser
- [结构化解析器](structured-parser.md) -- 将 JSON 反序列化为类型化的 Rust 结构体
- [枚举解析器](enum-parser.md) -- 验证输出是否在固定值集合中
