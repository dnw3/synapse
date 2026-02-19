# Few-Shot 提示

`FewShotChatMessagePromptTemplate` 将示例对话注入到提示词中，用于少样本学习。每个示例是一对人类输入和 AI 输出，格式化为交替的 `Human` 和 `AI` 消息。可以选择性地在前面添加系统前缀消息。

## 基本用法

使用 `FewShotExample` 值列表和一个用于用户实际查询的后缀 `PromptTemplate` 来创建模板：

```rust
use std::collections::HashMap;
use synaptic_prompts::{
    FewShotChatMessagePromptTemplate, FewShotExample, PromptTemplate,
};

let template = FewShotChatMessagePromptTemplate::new(
    vec![
        FewShotExample {
            input: "What is 2+2?".to_string(),
            output: "4".to_string(),
        },
        FewShotExample {
            input: "What is 3+3?".to_string(),
            output: "6".to_string(),
        },
    ],
    PromptTemplate::new("{{ question }}"),
);

let values = HashMap::from([
    ("question".to_string(), "What is 4+4?".to_string()),
]);
let messages = template.format(&values).unwrap();

// messages[0] => Human("What is 2+2?")  -- example 1 input
// messages[1] => AI("4")                 -- example 1 output
// messages[2] => Human("What is 3+3?")  -- example 2 input
// messages[3] => AI("6")                 -- example 2 output
// messages[4] => Human("What is 4+4?")  -- actual query (suffix)
```

每个 `FewShotExample` 有两个字段：

- `input` -- 该示例的人类消息
- `output` -- 该示例的 AI 响应

`suffix` 模板使用用户提供的变量渲染，并作为最后一条人类消息追加。

## 添加系统前缀

使用 `with_prefix()` 在示例之前添加系统消息：

```rust
use std::collections::HashMap;
use synaptic_prompts::{
    FewShotChatMessagePromptTemplate, FewShotExample, PromptTemplate,
};

let template = FewShotChatMessagePromptTemplate::new(
    vec![FewShotExample {
        input: "hi".to_string(),
        output: "hello".to_string(),
    }],
    PromptTemplate::new("{{ input }}"),
)
.with_prefix(PromptTemplate::new("You are a polite assistant."));

let values = HashMap::from([("input".to_string(), "hey".to_string())]);
let messages = template.format(&values).unwrap();

// messages[0] => System("You are a polite assistant.")  -- prefix
// messages[1] => Human("hi")                            -- example input
// messages[2] => AI("hello")                            -- example output
// messages[3] => Human("hey")                           -- actual query
```

前缀模板支持 `{{ variable }}` 插值，因此你也可以参数化系统消息。

## 作为 Runnable 使用

`FewShotChatMessagePromptTemplate` 实现了 `Runnable<HashMap<String, String>, Vec<Message>>`，因此你可以调用 `invoke()` 或在管道中进行组合：

```rust
use std::collections::HashMap;
use synaptic_core::RunnableConfig;
use synaptic_prompts::{
    FewShotChatMessagePromptTemplate, FewShotExample, PromptTemplate,
};
use synaptic_runnables::Runnable;

let template = FewShotChatMessagePromptTemplate::new(
    vec![FewShotExample {
        input: "x".to_string(),
        output: "y".to_string(),
    }],
    PromptTemplate::new("{{ q }}"),
);

let config = RunnableConfig::default();
let values = HashMap::from([("q".to_string(), "z".to_string())]);
let messages = template.invoke(values, &config).await?;
// 3 messages: Human("x"), AI("y"), Human("z")
```

> **注意：** `FewShotChatMessagePromptTemplate` 的 `Runnable` 实现接受 `HashMap<String, String>`，而 `ChatPromptTemplate` 接受 `HashMap<String, serde_json::Value>`。这种差异反映了它们底层模板渲染的不同：few-shot 模板使用 `PromptTemplate::render()`，它处理字符串值。
