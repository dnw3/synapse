use std::sync::Arc;

use async_trait::async_trait;
use colored::Colorize;
use synaptic::core::SynapticError;
use synaptic_deep::{SkillHookEvent, SkillHooksExecutor};

use crate::config::SynapseConfig;

/// Synapse product-layer implementation of skill lifecycle hooks.
///
/// Handles PreToolUse, PostToolUse, and Stop events emitted by the
/// framework during skill execution.
pub struct SynapseHooksExecutor {
    _config: Arc<SynapseConfig>,
}

impl SynapseHooksExecutor {
    pub fn new(config: Arc<SynapseConfig>) -> Self {
        Self { _config: config }
    }
}

#[async_trait]
impl SkillHooksExecutor for SynapseHooksExecutor {
    async fn execute_hook(&self, event: SkillHookEvent) -> Result<bool, SynapticError> {
        match event {
            SkillHookEvent::PreToolUse {
                skill_name,
                tool_name,
                ..
            } => {
                tracing::debug!(
                    skill = %skill_name,
                    tool = %tool_name,
                    "skill hook: pre-tool-use"
                );
                Ok(true)
            }
            SkillHookEvent::PostToolUse {
                skill_name,
                tool_name,
                ..
            } => {
                tracing::debug!(
                    skill = %skill_name,
                    tool = %tool_name,
                    "skill hook: post-tool-use"
                );
                Ok(true)
            }
            SkillHookEvent::Stop { skill_name, reason } => {
                eprintln!(
                    "{} Skill '{}' stopped: {}",
                    "hooks:".blue().bold(),
                    skill_name,
                    reason
                );
                Ok(true)
            }
        }
    }
}
