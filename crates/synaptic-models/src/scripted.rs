use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, SynapticError};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ScriptedChatModel {
    responses: Arc<Mutex<VecDeque<ChatResponse>>>,
}

impl ScriptedChatModel {
    pub fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }
}

#[async_trait]
impl ChatModel for ScriptedChatModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let mut responses = self.responses.lock().await;
        responses
            .pop_front()
            .ok_or_else(|| SynapticError::Model("scripted model exhausted responses".to_string()))
    }
}
