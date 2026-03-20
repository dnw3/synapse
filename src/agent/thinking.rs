use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{RunContext, SynapticError, ThinkingLevel};
use synaptic::middleware::{Interceptor, ModelCaller, ModelRequest, ModelResponse};
use tokio::sync::RwLock;

/// Interceptor that injects ThinkingLevel into every ModelRequest.
///
/// Supports both static thinking level and adaptive mode that adjusts
/// based on conversation complexity.
pub struct ThinkingMiddleware {
    level: Arc<RwLock<Option<ThinkingLevel>>>,
    adaptive: bool,
}

impl ThinkingMiddleware {
    /// Create with a fixed thinking level.
    pub fn new(level: Option<ThinkingLevel>) -> Self {
        Self {
            level: Arc::new(RwLock::new(level)),
            adaptive: false,
        }
    }

    /// Create in adaptive mode — adjusts thinking level based on complexity.
    pub fn adaptive() -> Self {
        Self {
            level: Arc::new(RwLock::new(None)),
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

    /// Map complexity score to ThinkingLevel.
    fn complexity_to_level(score: u32) -> Option<ThinkingLevel> {
        match score {
            0..=2 => None, // simple — no thinking
            3..=5 => Some(ThinkingLevel::Low),
            6..=8 => Some(ThinkingLevel::Medium),
            _ => Some(ThinkingLevel::High),
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
            Self::complexity_to_level(score)
        } else {
            self.level.read().await.clone()
        };

        if let Some(level) = thinking {
            request.thinking = Some(level);
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

/// Parse a thinking level string into a ThinkingLevel.
pub fn parse_thinking_level(level: &str) -> Option<ThinkingLevel> {
    match level {
        "off" => Some(ThinkingLevel::Off),
        "none" | "" => None,
        "low" | "minimal" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "adaptive" => None, // handled by AdaptiveThinkingMiddleware
        other => other.parse::<u32>().ok().map(ThinkingLevel::Budget),
    }
}
