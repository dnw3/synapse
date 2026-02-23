# 安全分析

安全中间件评估工具调用的风险等级，并在执行危险操作前可选地要求用户确认。

## 风险等级

```rust,ignore
use synaptic::middleware::RiskLevel;

pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}
```

## SecurityAnalyzer Trait

根据工具名称和参数评估工具调用的风险等级。

```rust,ignore
use synaptic::middleware::SecurityAnalyzer;

#[async_trait]
pub trait SecurityAnalyzer: Send + Sync {
    async fn assess(&self, tool_name: &str, args: &Value) -> Result<RiskLevel, SynapticError>;
}
```

### RuleBasedAnalyzer

将工具名称和参数模式映射到风险等级。

```rust,ignore
use synaptic::middleware::{RuleBasedAnalyzer, RiskLevel};

let analyzer = RuleBasedAnalyzer::new()
    .with_default_risk(RiskLevel::Low)
    .with_tool_risk("delete_file", RiskLevel::High)
    .with_tool_risk("read_file", RiskLevel::None)
    .with_arg_pattern("path", "/etc", RiskLevel::Critical);
```

参数模式在工具参数值包含指定子串时会提升风险等级。

## ConfirmationPolicy

判断给定风险等级的工具调用是否需要用户确认。

```rust,ignore
use synaptic::middleware::{ThresholdConfirmationPolicy, RiskLevel};

// 对 High 和 Critical 风险要求确认
let policy = ThresholdConfirmationPolicy::new(RiskLevel::High);
```

## SecurityConfirmationCallback

实现此 trait 来定义如何向用户获取确认。

```rust,ignore
use synaptic::middleware::{SecurityConfirmationCallback, RiskLevel};

struct CliConfirmation;

#[async_trait]
impl SecurityConfirmationCallback for CliConfirmation {
    async fn confirm(
        &self,
        tool_name: &str,
        args: &Value,
        risk: RiskLevel,
    ) -> Result<bool, SynapticError> {
        println!("工具 '{}' 风险等级为 {:?}，是否允许？[y/N]", tool_name, risk);
        // 读取用户输入...
        Ok(true)
    }
}
```

## SecurityMiddleware

将分析器、策略和回调组合为一个中间件。

```rust,ignore
use synaptic::middleware::SecurityMiddleware;

let middleware = SecurityMiddleware::new(
    Arc::new(analyzer),
    Arc::new(policy),
    Arc::new(CliConfirmation),
)
.with_bypass(["get_weather"]);  // 这些工具跳过安全检查

let options = AgentOptions {
    middleware: vec![Arc::new(middleware)],
    ..Default::default()
};
```

当工具调用被拦截时，中间件评估其风险，检查策略，若需要确认则调用回调。如果用户拒绝，工具调用将返回错误。
