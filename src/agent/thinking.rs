use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{RunContext, SynapticError, ThinkingConfig};
use synaptic::middleware::{Interceptor, ModelCaller, ModelRequest, ModelResponse};
use tokio::sync::RwLock;

/// Interceptor that injects ThinkingConfig into every ModelRequest.
///
/// Supports both static thinking level and adaptive mode that adjusts
/// based on conversation complexity.
pub struct ThinkingMiddleware {
    config: Arc<RwLock<Option<ThinkingConfig>>>,
    adaptive: bool,
}

impl ThinkingMiddleware {
    /// Create with a fixed thinking config.
    pub fn new(config: Option<ThinkingConfig>) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            adaptive: false,
        }
    }

    /// Create in adaptive mode — adjusts thinking level based on complexity.
    pub fn adaptive() -> Self {
        Self {
            config: Arc::new(RwLock::new(None)),
            adaptive: true,
        }
    }

    /// Estimate conversation complexity from a ModelRequest.
    fn estimate_complexity(request: &ModelRequest) -> u32 {
        let mut score: u32 = 0;

        // Message count contributes
        let msg_count = request.messages.len();
        if msg_count > 20 {
            score += 3;
        } else if msg_count > 10 {
            score += 2;
        } else if msg_count > 5 {
            score += 1;
        }

        // Total content length
        let total_chars: usize = request.messages.iter().map(|m| m.content().len()).sum();
        if total_chars > 10000 {
            score += 3;
        } else if total_chars > 3000 {
            score += 2;
        } else if total_chars > 1000 {
            score += 1;
        }

        // Tool count indicates complexity
        if request.tools.len() > 10 {
            score += 2;
        } else if request.tools.len() > 3 {
            score += 1;
        }

        // Keywords in the last user message
        if let Some(last_human) = request.messages.iter().rev().find(|m| m.is_human()) {
            let content = last_human.content().to_lowercase();
            let complex_keywords = [
                "analyze",
                "debug",
                "refactor",
                "architect",
                "design",
                "implement",
                "optimize",
                "explain",
                "compare",
                "evaluate",
                "plan",
                "review",
                "分析",
                "调试",
                "重构",
                "设计",
                "实现",
                "优化",
            ];
            let matches = complex_keywords
                .iter()
                .filter(|k| content.contains(*k))
                .count();
            score += matches as u32;
        }

        score
    }

    /// Map complexity score to ThinkingConfig.
    fn complexity_to_config(score: u32) -> Option<ThinkingConfig> {
        match score {
            0..=2 => None, // simple — no thinking
            3..=5 => Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(2000),
            }),
            6..=8 => Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(10000),
            }),
            _ => Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(50000),
            }),
        }
    }
}

#[async_trait]
impl Interceptor for ThinkingMiddleware {
    async fn wrap_model_call(
        &self,
        mut request: ModelRequest,
        ctx: &RunContext,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        let thinking = if self.adaptive {
            let score = Self::estimate_complexity(&request);
            Self::complexity_to_config(score)
        } else {
            self.config.read().await.clone()
        };

        if let Some(tc) = thinking {
            request.thinking = Some(tc);
        }

        next.call(request, ctx).await
    }
}

/// Interceptor that injects verbose/concise instructions into the system prompt.
pub struct VerboseMiddleware {
    level: String, // "off", "on", "full", "inherit"
}

impl VerboseMiddleware {
    pub fn new(level: &str) -> Self {
        Self {
            level: level.to_string(),
        }
    }
}

#[async_trait]
impl Interceptor for VerboseMiddleware {
    async fn wrap_model_call(
        &self,
        mut request: ModelRequest,
        ctx: &RunContext,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        let suffix = match self.level.as_str() {
            "off" => Some(
                "\n\n[INSTRUCTION: Be extremely concise. Answer directly without explanation. \
                 Omit filler words, preamble, and unnecessary transitions.]",
            ),
            "full" => Some(
                "\n\n[INSTRUCTION: Explain your reasoning step by step. Be thorough and detailed. \
                 Show your work and consider edge cases.]",
            ),
            _ => None, // "on" or "inherit" — default behavior
        };

        if let Some(suffix) = suffix {
            if let Some(ref mut prompt) = request.system_prompt {
                prompt.push_str(suffix);
            } else {
                request.system_prompt = Some(suffix.to_string());
            }
        }

        next.call(request, ctx).await
    }
}

/// Parse a thinking level string into a ThinkingConfig.
pub fn parse_thinking_level(level: &str) -> Option<ThinkingConfig> {
    match level {
        "off" | "none" | "" => None,
        "low" | "minimal" => Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(2000),
        }),
        "medium" => Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(10000),
        }),
        "high" => Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(50000),
        }),
        "adaptive" => None, // handled by AdaptiveThinkingMiddleware
        _ => {
            // Try parsing as a number (custom budget)
            level.parse::<u32>().ok().map(|budget| ThinkingConfig {
                enabled: true,
                budget_tokens: Some(budget),
            })
        }
    }
}
