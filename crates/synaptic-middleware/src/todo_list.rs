use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::{Message, SynapticError};
use tokio::sync::Mutex;

use crate::{AgentMiddleware, ModelRequest};

/// A single task in the agent's todo list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: usize,
    pub task: String,
    pub done: bool,
}

/// Adds task-planning capability to an agent by injecting a todo list
/// into the system prompt.
///
/// The middleware maintains a shared todo list. Before each model call,
/// it appends the current todo state to the system prompt, giving the
/// model awareness of remaining tasks.
pub struct TodoListMiddleware {
    items: Arc<Mutex<Vec<TodoItem>>>,
    next_id: Arc<Mutex<usize>>,
}

impl TodoListMiddleware {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Add a task to the todo list.
    pub async fn add(&self, task: impl Into<String>) -> usize {
        let mut id = self.next_id.lock().await;
        let item_id = *id;
        *id += 1;
        drop(id);

        let item = TodoItem {
            id: item_id,
            task: task.into(),
            done: false,
        };
        self.items.lock().await.push(item);
        item_id
    }

    /// Mark a task as done.
    pub async fn complete(&self, id: usize) -> bool {
        let mut items = self.items.lock().await;
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.done = true;
            true
        } else {
            false
        }
    }

    /// Get all items.
    pub async fn items(&self) -> Vec<TodoItem> {
        self.items.lock().await.clone()
    }

    fn format_list(items: &[TodoItem]) -> String {
        if items.is_empty() {
            return "No tasks in the todo list.".to_string();
        }
        let mut s = String::from("Current TODO list:\n");
        for item in items {
            let mark = if item.done { "x" } else { " " };
            s.push_str(&format!("  [{}] #{}: {}\n", mark, item.id, item.task));
        }
        s
    }
}

impl Default for TodoListMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentMiddleware for TodoListMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        let items = self.items.lock().await;
        if items.is_empty() {
            return Ok(());
        }
        let list_text = Self::format_list(&items);
        drop(items);

        // Inject at the beginning of messages as a system message
        request.messages.insert(0, Message::system(list_text));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_and_complete() {
        let mw = TodoListMiddleware::new();
        let id1 = mw.add("Write tests").await;
        let id2 = mw.add("Fix bug").await;
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        assert!(mw.complete(1).await);
        let items = mw.items().await;
        assert!(items[0].done);
        assert!(!items[1].done);
    }

    #[tokio::test]
    async fn format_empty() {
        let text = TodoListMiddleware::format_list(&[]);
        assert_eq!(text, "No tasks in the todo list.");
    }
}
