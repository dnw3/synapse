# Security Analysis

The security middleware assesses tool call risk and optionally requires user confirmation before executing dangerous operations.

## Risk Levels

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

Assesses the risk level of a tool call based on its name and arguments.

```rust,ignore
use synaptic::middleware::SecurityAnalyzer;

#[async_trait]
pub trait SecurityAnalyzer: Send + Sync {
    async fn assess(&self, tool_name: &str, args: &Value) -> Result<RiskLevel, SynapticError>;
}
```

### RuleBasedAnalyzer

Maps tool names and argument patterns to risk levels.

```rust,ignore
use synaptic::middleware::{RuleBasedAnalyzer, RiskLevel};

let analyzer = RuleBasedAnalyzer::new()
    .with_default_risk(RiskLevel::Low)
    .with_tool_risk("delete_file", RiskLevel::High)
    .with_tool_risk("read_file", RiskLevel::None)
    .with_arg_pattern("path", "/etc", RiskLevel::Critical);
```

Argument patterns elevate the risk when a tool argument value contains the specified substring.

## ConfirmationPolicy

Determines whether a tool call at a given risk level requires user confirmation.

```rust,ignore
use synaptic::middleware::{ThresholdConfirmationPolicy, RiskLevel};

// Require confirmation for High and Critical risk
let policy = ThresholdConfirmationPolicy::new(RiskLevel::High);
```

## SecurityConfirmationCallback

Implement this trait to define how confirmation is obtained from the user.

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
        println!("Tool '{}' has {:?} risk. Allow? [y/N]", tool_name, risk);
        // read user input...
        Ok(true)
    }
}
```

## SecurityMiddleware

Combines the analyzer, policy, and callback into a single middleware.

```rust,ignore
use synaptic::middleware::SecurityMiddleware;

let middleware = SecurityMiddleware::new(
    Arc::new(analyzer),
    Arc::new(policy),
    Arc::new(CliConfirmation),
)
.with_bypass(["get_weather"]);  // these tools skip security checks

let options = AgentOptions {
    middleware: vec![Arc::new(middleware)],
    ..Default::default()
};
```

When a tool call is intercepted, the middleware assesses its risk, checks the policy, and if confirmation is required, invokes the callback. If the user rejects, the tool call returns an error.
