use std::sync::Arc;

use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{ChatModel, Message, ThinkingConfig};

use crate::memory::LongTermMemory;

/// Mutable state for the REPL session.
pub struct ReplState {
    pub model: Arc<dyn ChatModel>,
    pub current_model_name: String,
    pub current_session_id: String,
    pub messages: Vec<Message>,
    pub tracker: Arc<CostTrackingCallback>,
    pub ltm: Arc<LongTermMemory>,
    pub verbose: bool,
    pub thinking: Option<ThinkingConfig>,
}
