use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// LRU-based message deduplication.
///
/// The Lark long-connection listener has its own in-memory dedup, but it resets
/// on process restart. This provides an additional business-layer dedup that
/// can catch duplicates across reconnections within the LRU window.
pub struct MessageDedup {
    cache: Mutex<LruCache<String, ()>>,
}

impl MessageDedup {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1024).unwrap()),
            )),
        }
    }

    /// Returns true if this event_id has NOT been seen before (i.e., is new).
    /// Returns false if it's a duplicate.
    pub fn check_and_mark(&self, event_id: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if cache.contains(event_id) {
            false
        } else {
            cache.put(event_id.to_string(), ());
            true
        }
    }
}
