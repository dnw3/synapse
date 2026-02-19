# Runnables (LCEL)

Synaptic 通过 `Runnable` trait 和一组可组合的构建模块实现了 LCEL（LangChain Expression Language）。LCEL 链中的每个组件——提示词、模型、解析器、自定义逻辑——都实现了相同的 `Runnable<I, O>` 接口，因此可以通过统一的 API 自由组合。

## `Runnable` trait

`Runnable<I, O>` trait 定义在 `synaptic_core` 中，提供三个核心方法：

| 方法 | 描述 |
|--------|-------------|
| `invoke(input, config)` | 对单个输入执行，返回一个输出 |
| `batch(inputs, config)` | 对多个输入顺序执行 |
| `stream(input, config)` | 返回一个增量结果的 `RunnableOutputStream` |

每个 `Runnable` 还有一个 `boxed()` 方法，可以将其包装为 `BoxRunnable<I, O>`——一个类型擦除的容器，支持使用 `|` pipe 运算符进行组合。

```rust
use synaptic_runnables::{Runnable, RunnableLambda, BoxRunnable};
use synaptic_core::RunnableConfig;

let step = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
});

let config = RunnableConfig::default();
let result = step.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "HELLO");
```

## `BoxRunnable`——类型擦除的组合

`BoxRunnable<I, O>` 是构建链的关键类型。它将任意 `Runnable<I, O>` 包装在 trait 对象之后，从而擦除具体类型。这是必要的，因为 `|` 运算符要求调用位置两侧具有已知的类型。

`BoxRunnable` 本身也实现了 `Runnable<I, O>`，因此 boxed 的 runnable 可以无缝组合。

## 构建模块

Synaptic 提供以下 LCEL 构建模块：

| 类型 | 用途 |
|------|---------|
| `RunnableLambda` | 将异步闭包包装为 runnable |
| `RunnablePassthrough` | 原样传递输入 |
| `RunnableSequence` | 串联两个 runnable（由 `|` 运算符创建） |
| `RunnableParallel` | 并发运行命名分支，合并为 JSON |
| `RunnableBranch` | 根据条件路由输入，带有默认回退 |
| `RunnableAssign` | 将并行分支结果合并到输入 JSON 对象中 |
| `RunnablePick` | 从 JSON 对象中提取指定的键 |
| `RunnableWithFallbacks` | 当主 runnable 失败时尝试替代方案 |
| `RunnableRetry` | 失败时使用指数退避进行重试 |
| `RunnableEach` | 对 `Vec` 中的每个元素映射执行一个 runnable |
| `RunnableGenerator` | 将生成器函数包装为支持真正流式输出的 runnable |

## 指南

- [Pipe 运算符](pipe-operator.md)——使用 `|` 串联 runnable 以构建顺序管道
- [流式处理](streaming.md)——通过链消费增量输出
- [Parallel 与 Branch](parallel-branch.md)——并发运行分支或根据条件路由
- [Assign 与 Pick](assign-pick.md)——将计算的键合并到 JSON 中并提取特定字段
- [Fallbacks](fallbacks.md)——在主 runnable 失败时提供替代 runnable
- [Bind](bind.md)——为 runnable 附加配置变换
- [Retry](retry.md)——在瞬态故障时使用指数退避重试
- [Generator](generator.md)——将流式生成器函数包装为 runnable
- [Each](each.md)——对列表中的每个元素映射执行一个 runnable
