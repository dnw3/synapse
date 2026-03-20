use super::inbound::InboundMessage;
use std::collections::HashMap;

/// LRU-based inbound message deduplicator.
#[allow(dead_code)]
pub struct InboundDeduplicator {
    cache: HashMap<String, u64>,
    ttl_ms: u64,
    max_size: usize,
}

#[allow(dead_code)]
impl InboundDeduplicator {
    pub fn new(ttl_ms: u64, max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            ttl_ms,
            max_size,
        }
    }

    /// Returns true if this message is a duplicate (should be skipped).
    pub fn is_duplicate(&mut self, msg: &InboundMessage) -> bool {
        let key = self.composite_key(msg);
        let now = msg.timestamp_ms;

        // Evict expired entries
        self.cache
            .retain(|_, ts| now.saturating_sub(*ts) < self.ttl_ms);

        // Check for duplicate
        if let Some(&ts) = self.cache.get(&key) {
            if now.saturating_sub(ts) < self.ttl_ms {
                return true;
            }
        }

        // Evict oldest if at capacity
        if self.cache.len() >= self.max_size {
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(_, ts)| *ts)
                .map(|(k, _)| k.clone())
            {
                self.cache.remove(&oldest_key);
            }
        }

        self.cache.insert(key, now);
        false
    }

    fn composite_key(&self, msg: &InboundMessage) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            msg.channel.platform,
            msg.channel.account_id.as_deref().unwrap_or(""),
            msg.session_key,
            msg.sender.id.as_deref().unwrap_or(""),
            msg.message
                .id
                .as_deref()
                .unwrap_or(msg.idempotency_key.as_deref().unwrap_or("")),
        )
    }
}

impl Default for InboundDeduplicator {
    fn default() -> Self {
        Self::new(60_000, 1000) // 1 minute TTL, 1000 entries max
    }
}
