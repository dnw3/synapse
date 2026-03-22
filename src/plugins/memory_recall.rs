//! MemoryRecallInterceptor — auto-recalls relevant memories before each model call.
//!
//! Works with any MemoryProvider implementation (native LTM or Viking).
//! Pattern: Interceptor (request mutation), not EventSubscriber (observation).

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{Message, SynapticError};
use synaptic::memory::{MemoryProvider, MemoryResult};
use synaptic::middleware::{Interceptor, ModelRequest};

pub struct MemoryRecallInterceptor {
    provider: Arc<dyn MemoryProvider>,
    limit: usize,
    score_threshold: f64,
}

impl MemoryRecallInterceptor {
    pub fn new(provider: Arc<dyn MemoryProvider>, limit: usize, score_threshold: f64) -> Self {
        Self {
            provider,
            limit,
            score_threshold,
        }
    }
}

#[async_trait]
impl Interceptor for MemoryRecallInterceptor {
    async fn before_model(&self, req: &mut ModelRequest) -> Result<(), SynapticError> {
        let query = extract_last_user_message(&req.messages);

        if should_skip_recall(&query) {
            return Ok(());
        }

        let results = match self.provider.recall(&query, self.limit).await {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!(error = %e, "memory recall failed, continuing without memories");
                return Ok(()); // Don't block on memory failures
            }
        };

        let results: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= self.score_threshold)
            .collect();

        if results.is_empty() {
            return Ok(());
        }

        let recall_text = format_recall_results(&results);
        let section = format!("\n\n<recalled_memories>\n{recall_text}\n</recalled_memories>");

        match req.system_prompt {
            Some(ref mut prompt) => prompt.push_str(&section),
            None => req.system_prompt = Some(section),
        }

        tracing::debug!(
            count = results.len(),
            "injected recalled memories into prompt"
        );
        Ok(())
    }
}

/// Extract the last user (human) message text from the message list.
fn extract_last_user_message(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.role() == "human")
        .map(|m| m.content().to_string())
        .unwrap_or_default()
}

/// Skip recall for greetings and very short messages.
fn should_skip_recall(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.chars().count() <= 2 {
        return true;
    }
    const GREETINGS: &[&str] = &[
        "你好",
        "hi",
        "hello",
        "hey",
        "嗨",
        "哈喽",
        "早上好",
        "晚上好",
        "good morning",
        "good evening",
        "thanks",
        "谢谢",
        "ok",
    ];
    GREETINGS.iter().any(|g| trimmed.eq_ignore_ascii_case(g))
}

/// Format recall results for injection into system prompt.
fn format_recall_results(results: &[MemoryResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let cat = r.category.as_deref().unwrap_or("general");
            format!(
                "{}. [{}] {} (score: {:.2}, uri: {})",
                i + 1,
                cat,
                r.content,
                r.score,
                r.uri
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_short_messages() {
        assert!(should_skip_recall(""));
        assert!(should_skip_recall("hi"));
        assert!(should_skip_recall("ok"));
        assert!(should_skip_recall("  你好  "));
    }

    #[test]
    fn do_not_skip_real_queries() {
        assert!(!should_skip_recall("how do I authenticate?"));
        assert!(!should_skip_recall("写一个排序算法"));
        assert!(!should_skip_recall("what was that bug we fixed last week?"));
    }

    #[test]
    fn extract_last_human_message() {
        let messages = vec![
            Message::human("first question"),
            Message::ai("first answer"),
            Message::human("second question"),
        ];
        assert_eq!(extract_last_user_message(&messages), "second question");
    }

    #[test]
    fn extract_from_empty_messages() {
        let messages: Vec<Message> = vec![];
        assert_eq!(extract_last_user_message(&messages), "");
    }

    #[test]
    fn format_results() {
        let results = vec![MemoryResult {
            uri: "ltm:0".into(),
            content: "User prefers dark mode".into(),
            score: 0.85,
            category: Some("preferences".into()),
            layer: None,
            metadata: serde_json::Value::Null,
        }];
        let text = format_recall_results(&results);
        assert!(text.contains("[preferences]"));
        assert!(text.contains("dark mode"));
        assert!(text.contains("0.85"));
    }
}
