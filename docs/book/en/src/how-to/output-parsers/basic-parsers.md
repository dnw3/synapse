# Basic Parsers

Synaptic provides several simple output parsers for common transformations. Each implements `Runnable`, so it can be used standalone or composed in a pipeline.

## StrOutputParser

Extracts the text content from a `Message`. This is the most commonly used parser -- it sits at the end of most chains to convert the model's response into a plain `String`.

**Signature:** `Runnable<Message, String>`

```rust
use synaptic_parsers::StrOutputParser;
use synaptic_runnables::Runnable;
use synaptic_core::{Message, RunnableConfig};

let parser = StrOutputParser;
let config = RunnableConfig::default();

let result = parser.invoke(Message::ai("Hello world"), &config).await?;
assert_eq!(result, "Hello world");
```

`StrOutputParser` works with any `Message` variant -- system, human, AI, or tool messages all have content that can be extracted.

## JsonOutputParser

Parses a JSON string into a `serde_json::Value`. Useful when you need to work with arbitrary JSON structures without defining a specific Rust type.

**Signature:** `Runnable<String, serde_json::Value>`

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

If the input is not valid JSON, the parser returns `Err(SynapticError::Parsing(...))`.

## ListOutputParser

Splits a string into a `Vec<String>` using a configurable separator. Useful when you ask the LLM to return a comma-separated or newline-separated list.

**Signature:** `Runnable<String, Vec<String>>`

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

Each item is trimmed of leading and trailing whitespace. Empty items after trimming are filtered out.

## BooleanOutputParser

Parses yes/no, true/false, y/n, and 1/0 style responses into a `bool`. Case-insensitive and whitespace-trimmed.

**Signature:** `Runnable<String, bool>`

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

Unrecognized values return `Err(SynapticError::Parsing(...))`.

## XmlOutputParser

Parses XML-formatted LLM output into an `XmlElement` tree. Supports nested elements, attributes, and text content without requiring a full XML library.

**Signature:** `Runnable<String, XmlElement>`

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

Use `XmlOutputParser::new()` to parse the entire input as XML, or `with_root_tag("tag")` to extract content from within a specific root tag.

## MarkdownListOutputParser

Parses markdown-formatted bullet lists (`- item` or `* item`) into a `Vec<String>`. Lines not starting with a bullet marker are ignored.

**Signature:** `Runnable<String, Vec<String>>`

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

Parses numbered lists (`1. item`, `2. item`) into a `Vec<String>`. The number prefix is stripped; only lines matching the `N. text` pattern are included.

**Signature:** `Runnable<String, Vec<String>>`

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

## Format Instructions

All parsers implement the `FormatInstructions` trait. You can include the instructions in your prompt to guide the model:

```rust
use synaptic_parsers::{JsonOutputParser, ListOutputParser, FormatInstructions};

let json_parser = JsonOutputParser;
println!("{}", json_parser.get_format_instructions());
// "Your response should be a valid JSON object."

let list_parser = ListOutputParser::comma();
println!("{}", list_parser.get_format_instructions());
// "Your response should be a list of items separated by commas."
```

## Pipeline Example

A typical chain pipes a prompt template through a model and into a parser:

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
