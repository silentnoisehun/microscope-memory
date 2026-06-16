//! Tiered LRU cache for Microscope Memory queries.
//!
//! Two tiers:
//! - **Tier 1 (hot)**: Exact query → JSON response cache
//! - **Tier 2 (warm)**: Block index → text cache (avoids repeat mmap reads)
//!
//! Thread-safe via `Mutex`. Cache is invalidated on store/rebuild.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A single cached entry with TTL tracking.
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    hits: u64,
}

/// Fixed-capacity LRU cache with TTL expiration.
struct LruTier<V> {
    entries: Vec<(String, CacheEntry<V>)>,
    capacity: usize,
    ttl: Duration,
}

impl<V: Clone> LruTier<V> {
    fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            ttl,
        }
    }

    fn get(&mut self, key: &str) -> Option<V> {
        let now = Instant::now();
        // Find and check TTL
        let pos = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = &mut self.entries[pos].1;
        if now.duration_since(entry.created_at) > self.ttl {
            self.entries.remove(pos);
            return None;
        }
        entry.hits += 1;
        let value = entry.value.clone();
        // Move to end (most recently used)
        let item = self.entries.remove(pos);
        self.entries.push(item);
        Some(value)
    }

    fn insert(&mut self, key: String, value: V) {
        // Remove existing entry with same key
        self.entries.retain(|(k, _)| k != &key);
        // Evict LRU if at capacity
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((
            key,
            CacheEntry {
                value,
                created_at: Instant::now(),
                hits: 0,
            },
        ));
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn total_hits(&self) -> u64 {
        self.entries.iter().map(|(_, e)| e.hits).sum()
    }
}

/// Thread-safe two-tier query cache.
pub struct QueryCache {
    /// Tier 1: query key → JSON response
    tier1: Mutex<LruTier<String>>,
    /// Tier 2: block index → text content
    tier2: Mutex<LruTier<String>>,
}

/// Cache statistics for the /stats endpoint.
pub struct CacheStats {
    pub tier1_entries: usize,
    pub tier1_hits: u64,
    pub tier2_entries: usize,
    pub tier2_hits: u64,
}

impl QueryCache {
    /// Create a new cache with given capacities and TTL in seconds.
    pub fn new(tier1_capacity: usize, tier2_capacity: usize, ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        Self {
            tier1: Mutex::new(LruTier::new(tier1_capacity, ttl)),
            tier2: Mutex::new(LruTier::new(tier2_capacity, ttl)),
        }
    }

    // ─── Tier 1: query results ────────────────────────

    /// Look up a cached query response. Key format: "endpoint:query:k"
    pub fn get_query(&self, key: &str) -> Option<String> {
        self.tier1.lock().ok()?.get(key)
    }

    /// Cache a query response.
    pub fn insert_query(&self, key: String, json_response: String) {
        if let Ok(mut t) = self.tier1.lock() {
            t.insert(key, json_response);
        }
    }

    // ─── Tier 2: block text ───────────────────────────

    /// Look up cached block text by index.
    pub fn get_block_text(&self, block_idx: usize) -> Option<String> {
        let key = block_idx.to_string();
        self.tier2.lock().ok()?.get(&key)
    }

    /// Cache block text by index.
    pub fn insert_block_text(&self, block_idx: usize, text: String) {
        if let Ok(mut t) = self.tier2.lock() {
            t.insert(block_idx.to_string(), text);
        }
    }

    // ─── Cache management ─────────────────────────────

    /// Invalidate all cached entries (called on store/rebuild).
    pub fn invalidate_all(&self) {
        if let Ok(mut t) = self.tier1.lock() {
            t.clear();
        }
        if let Ok(mut t) = self.tier2.lock() {
            t.clear();
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let (t1_entries, t1_hits) = self
            .tier1
            .lock()
            .map(|t| (t.len(), t.total_hits()))
            .unwrap_or((0, 0));
        let (t2_entries, t2_hits) = self
            .tier2
            .lock()
            .map(|t| (t.len(), t.total_hits()))
            .unwrap_or((0, 0));
        CacheStats {
            tier1_entries: t1_entries,
            tier1_hits: t1_hits,
            tier2_entries: t2_entries,
            tier2_hits: t2_hits,
        }
    }

    /// Build a cache key for a query endpoint.
    pub fn make_key(endpoint: &str, query: &str, k: usize) -> String {
        format!("{}:{}:{}", endpoint, query.to_lowercase().trim(), k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_get() {
        let cache = QueryCache::new(4, 4, 300);
        cache.insert_query("recall:test:10".into(), "{\"results\":[]}".into());
        assert_eq!(
            cache.get_query("recall:test:10"),
            Some("{\"results\":[]}".into())
        );
        assert_eq!(cache.get_query("recall:missing:10"), None);
    }

    #[test]
    fn test_lru_eviction() {
        let cache = QueryCache::new(2, 2, 300);
        cache.insert_query("a".into(), "1".into());
        cache.insert_query("b".into(), "2".into());
        cache.insert_query("c".into(), "3".into()); // evicts "a"
        assert_eq!(cache.get_query("a"), None);
        assert_eq!(cache.get_query("b"), Some("2".into()));
        assert_eq!(cache.get_query("c"), Some("3".into()));
    }

    #[test]
    fn test_invalidate_all() {
        let cache = QueryCache::new(4, 4, 300);
        cache.insert_query("x".into(), "1".into());
        cache.insert_block_text(42, "hello".into());
        cache.invalidate_all();
        assert_eq!(cache.get_query("x"), None);
        assert_eq!(cache.get_block_text(42), None);
    }

    #[test]
    fn test_tier2_block_text() {
        let cache = QueryCache::new(4, 4, 300);
        cache.insert_block_text(0, "block zero".into());
        cache.insert_block_text(99, "block 99".into());
        assert_eq!(cache.get_block_text(0), Some("block zero".into()));
        assert_eq!(cache.get_block_text(99), Some("block 99".into()));
        assert_eq!(cache.get_block_text(1), None);
    }

    #[test]
    fn test_stats() {
        let cache = QueryCache::new(4, 4, 300);
        cache.insert_query("a".into(), "1".into());
        cache.insert_query("b".into(), "2".into());
        let _ = cache.get_query("a"); // hit
        let _ = cache.get_query("a"); // hit
        let stats = cache.stats();
        assert_eq!(stats.tier1_entries, 2);
        assert_eq!(stats.tier1_hits, 2);
    }

    #[test]
    fn test_ttl_expiry() {
        let cache = QueryCache::new(4, 4, 0); // 0 second TTL
        cache.insert_query("x".into(), "1".into());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(cache.get_query("x"), None); // expired
    }
}
