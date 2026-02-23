use crate::token_counter::TokenCounter;
use crate::Message;
use std::sync::Arc;

/// Priority level for context slots. Lower values = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

impl Priority {
    pub const CRITICAL: Priority = Priority(0);
    pub const HIGH: Priority = Priority(64);
    pub const NORMAL: Priority = Priority(128);
    pub const LOW: Priority = Priority(192);
}

/// A slot of context to include in the budget.
pub struct ContextSlot {
    pub name: String,
    pub priority: Priority,
    pub messages: Vec<Message>,
    /// Minimum reserved tokens for this slot (guaranteed if budget allows).
    pub reserved_tokens: usize,
}

/// Assembles messages from multiple context slots within a token budget.
///
/// Slots are sorted by priority (lowest value = highest priority).
/// Higher-priority slots are included first. Lower-priority slots are
/// dropped if the budget is exceeded.
pub struct ContextBudget {
    max_tokens: usize,
    counter: Arc<dyn TokenCounter>,
}

impl ContextBudget {
    pub fn new(max_tokens: usize, counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            max_tokens,
            counter,
        }
    }

    /// Assemble messages from slots that fit within the token budget.
    ///
    /// Slots are processed in priority order (CRITICAL first, LOW last).
    /// Each slot's messages are included if they fit. Slots with
    /// `reserved_tokens > 0` are guaranteed inclusion (if total reserved
    /// fits within budget).
    pub fn assemble(&self, mut slots: Vec<ContextSlot>) -> Vec<Message> {
        // Sort by priority (lower value = higher priority)
        slots.sort_by_key(|s| s.priority);

        let mut result = Vec::new();
        let mut used_tokens = 0;

        for slot in slots {
            let slot_tokens = self.counter.count_messages(&slot.messages);

            if slot.reserved_tokens > 0 {
                // Reserved slots are always included (up to budget)
                if used_tokens + slot_tokens <= self.max_tokens {
                    used_tokens += slot_tokens;
                    result.extend(slot.messages);
                }
            } else if used_tokens + slot_tokens <= self.max_tokens {
                used_tokens += slot_tokens;
                result.extend(slot.messages);
            }
            // If doesn't fit and not reserved, skip
        }

        result
    }
}
