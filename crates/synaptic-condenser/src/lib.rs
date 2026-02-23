mod llm_summarizing;
mod middleware;
mod noop;
mod pipeline;
mod rolling;
mod token_budget;

pub use llm_summarizing::LlmSummarizingCondenser;
pub use middleware::CondenserMiddleware;
pub use noop::NoOpCondenser;
pub use pipeline::PipelineCondenser;
pub use rolling::RollingCondenser;
pub use token_budget::TokenBudgetCondenser;

use async_trait::async_trait;
use synaptic_core::{Message, SynapticError};

/// Trait for condensing (compressing) a message history.
#[async_trait]
pub trait Condenser: Send + Sync {
    /// Condense the given messages into a shorter form.
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError>;
}
