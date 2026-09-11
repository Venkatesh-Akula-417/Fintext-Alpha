//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — In-Memory Cache TTL & Capacity Governance (Suite #272)
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides thread-safe, memory-bounded, time-to-live (TTL) caching utilities
//! over `DashMap` to prevent unbounded heap memory growth, mitigate potential
//! memory leaks, and guarantee system stability in continuous production deployments.

use std::env;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use tracing::debug;

/// Global configuration settings for in-memory cache governance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfig {
    /// Default time-to-live for cache entries in seconds (default: 300s / 5min)
    pub default_ttl_secs: u64,
    /// Default maximum capacity bound before evicting oldest entries (default: 10,000)
    pub default_max_capacity: usize,
    /// Interval in seconds between periodic background purge sweeps (default: 60s)
    pub cleanup_interval_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 300,
            default_max_capacity: 10_000,
            cleanup_interval_secs: 60,
        }
    }
}

impl CacheConfig {
    /// Resolves cache configuration from environment variables or `config/config.yaml`.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        // 1. Parse YAML config if present
        let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.yaml".to_string());
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            let mut in_cache = false;
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("cache:") {
                    in_cache = true;
                    continue;
                }
                if in_cache && !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                    break;
                }
                if in_cache {
                    if let Some((k, v)) = trimmed.split_once(':') {
                        let key = k.trim();
                        let val = v.split('#').next().unwrap_or("").trim();
                        match key {
                            "default_ttl_secs" => {
                                if let Ok(n) = val.parse::<u64>() {
                                    cfg.default_ttl_secs = n;
                                }
                            }
                            "default_max_capacity" => {
                                if let Ok(n) = val.parse::<usize>() {
                                    cfg.default_max_capacity = n;
                                }
                            }
                            "cleanup_interval_secs" => {
                                if let Ok(n) = val.parse::<u64>() {
                                    cfg.cleanup_interval_secs = n;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // 2. Environment variables take highest precedence
        if let Ok(val) = env::var("CACHE_DEFAULT_TTL_SECS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.default_ttl_secs = n;
            }
        }
        if let Ok(val) = env::var("CACHE_MAX_CAPACITY") {
            if let Ok(n) = val.parse::<usize>() {
                cfg.default_max_capacity = n;
            }
        }
        if let Ok(val) = env::var("CACHE_CLEANUP_INTERVAL_SECS") {
            if let Ok(n) = val.parse::<u64>() {
                cfg.cleanup_interval_secs = n;
            }
        }

        cfg
    }
}

/// Generic, thread-safe, lock-free concurrent in-memory cache with TTL and capacity limits.
#[derive(Debug, Clone)]
pub struct TtlCache<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    entries: Arc<DashMap<K, (V, Instant)>>,
    ttl: Duration,
    max_capacity: usize,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Initialize a new `TtlCache` with explicit TTL duration and maximum capacity.
    pub fn new(ttl: Duration, max_capacity: usize) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            ttl,
            max_capacity: max_capacity.max(1),
        }
    }

    /// Initialize a new `TtlCache` with TTL specified in seconds.
    pub fn with_ttl_secs(ttl_secs: u64, max_capacity: usize) -> Self {
        Self::new(Duration::from_secs(ttl_secs), max_capacity)
    }

    /// Access configured TTL duration.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Access configured maximum capacity.
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Insert a key-value pair into the cache with the current timestamp.
    ///
    /// If capacity has been reached, expired entries are purged first.
    /// If the cache is still at capacity, the oldest entry by timestamp is evicted.
    pub fn insert(&self, key: K, value: V) {
        if self.entries.len() >= self.max_capacity && !self.entries.contains_key(&key) {
            // First purge expired items
            let purged = self.remove_expired();
            if self.entries.len() >= self.max_capacity {
                // If still at capacity, evict the oldest entry
                if let Some(oldest_key) = self
                    .entries
                    .iter()
                    .min_by_key(|entry| entry.value().1)
                    .map(|entry| entry.key().clone())
                {
                    self.entries.remove(&oldest_key);
                    debug!(
                        "[TtlCache] Evicted oldest entry at capacity bound (purged={}, max={})",
                        purged, self.max_capacity
                    );
                }
            }
        }

        self.entries.insert(key, (value, Instant::now()));
    }

    /// Retrieve a cached value only if it exists and has not expired past the TTL.
    ///
    /// If an expired entry is encountered, it is lazily removed and `None` is returned.
    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get(key) {
            if entry.value().1.elapsed() < self.ttl {
                return Some(entry.value().0.clone());
            }
            // Expired — remove lazily
            drop(entry);
            self.entries.remove(key);
        }
        None
    }

    /// Retrieve an existing unexpired value or insert the result of a generator function.
    pub fn get_or_insert_with<F>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(existing) = self.get(&key) {
            return existing;
        }
        let generated = f();
        self.insert(key, generated.clone());
        generated
    }

    /// Mutate an existing cached value in-place without altering its cached timestamp.
    ///
    /// Returns `true` if the entry exists and was unexpired, `false` otherwise.
    pub fn update<F>(&self, key: &K, f: F) -> bool
    where
        F: FnOnce(&mut V),
    {
        if let Some(mut entry) = self.entries.get_mut(key) {
            if entry.value().1.elapsed() < self.ttl {
                f(&mut entry.value_mut().0);
                return true;
            }
            drop(entry);
            self.entries.remove(key);
        }
        false
    }

    /// Remove all expired entries whose timestamp is older than the configured TTL.
    ///
    /// Returns the number of entries purged.
    pub fn remove_expired(&self) -> usize {
        let mut purged = 0;
        let ttl = self.ttl;
        self.entries.retain(|_, (_, instant)| {
            if instant.elapsed() >= ttl {
                purged += 1;
                false
            } else {
                true
            }
        });
        purged
    }

    /// Returns the current number of stored entries (including potentially un-purged expired ones).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if the cache contains an active (unexpired) entry for the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Remove a specific entry from the cache by key.
    pub fn remove(&self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|(_, (val, _))| val)
    }

    /// Invalidate and remove all entries in the cache.
    pub fn clear(&self) {
        self.entries.clear();
    }
}

impl<K, V> Default for TtlCache<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        let cfg = CacheConfig::default();
        Self::with_ttl_secs(cfg.default_ttl_secs, cfg.default_max_capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_cache_config_defaults() {
        let cfg = CacheConfig::default();
        assert_eq!(cfg.default_ttl_secs, 300);
        assert_eq!(cfg.default_max_capacity, 10_000);
        assert_eq!(cfg.cleanup_interval_secs, 60);
    }

    #[test]
    fn test_cache_config_env_overrides() {
        std::env::set_var("CACHE_DEFAULT_TTL_SECS", "120");
        std::env::set_var("CACHE_MAX_CAPACITY", "5000");
        std::env::set_var("CACHE_CLEANUP_INTERVAL_SECS", "30");

        let cfg = CacheConfig::from_env_or_config();

        std::env::remove_var("CACHE_DEFAULT_TTL_SECS");
        std::env::remove_var("CACHE_MAX_CAPACITY");
        std::env::remove_var("CACHE_CLEANUP_INTERVAL_SECS");

        assert_eq!(cfg.default_ttl_secs, 120);
        assert_eq!(cfg.default_max_capacity, 5000);
        assert_eq!(cfg.cleanup_interval_secs, 30);
    }

    #[test]
    fn test_ttl_cache_insert_and_get() {
        let cache: TtlCache<String, i32> = TtlCache::with_ttl_secs(10, 100);
        cache.insert("apple".to_string(), 42);
        assert_eq!(cache.get(&"apple".to_string()), Some(42));
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        assert!(cache.contains_key(&"apple".to_string()));
        assert_eq!(cache.get(&"banana".to_string()), None);
    }

    #[test]
    fn test_ttl_cache_expiration() {
        let cache: TtlCache<String, String> = TtlCache::new(Duration::from_millis(50), 100);
        cache.insert("session_1".to_string(), "active".to_string());
        assert_eq!(cache.get(&"session_1".to_string()), Some("active".to_string()));

        sleep(Duration::from_millis(70));
        // Lazy expiration on get()
        assert_eq!(cache.get(&"session_1".to_string()), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_ttl_cache_capacity_eviction() {
        let cache: TtlCache<String, i32> = TtlCache::with_ttl_secs(60, 3);
        cache.insert("k1".to_string(), 1);
        sleep(Duration::from_millis(10));
        cache.insert("k2".to_string(), 2);
        sleep(Duration::from_millis(10));
        cache.insert("k3".to_string(), 3);
        assert_eq!(cache.len(), 3);

        // Inserting a 4th key should evict the oldest entry ("k1")
        sleep(Duration::from_millis(10));
        cache.insert("k4".to_string(), 4);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&"k1".to_string()), None); // Evicted!
        assert_eq!(cache.get(&"k2".to_string()), Some(2));
        assert_eq!(cache.get(&"k3".to_string()), Some(3));
        assert_eq!(cache.get(&"k4".to_string()), Some(4));
    }

    #[test]
    fn test_ttl_cache_remove_expired() {
        let cache: TtlCache<String, i32> = TtlCache::new(Duration::from_millis(40), 100);
        cache.insert("k1".to_string(), 10);
        cache.insert("k2".to_string(), 20);

        sleep(Duration::from_millis(60));
        cache.insert("k3".to_string(), 30); // Inserted fresh

        let purged = cache.remove_expired();
        assert_eq!(purged, 2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&"k3".to_string()), Some(30));
    }

    #[test]
    fn test_ttl_cache_update() {
        let cache: TtlCache<String, i32> = TtlCache::with_ttl_secs(60, 100);
        cache.insert("counter".to_string(), 5);

        let updated = cache.update(&"counter".to_string(), |val| {
            *val += 10;
        });
        assert!(updated);
        assert_eq!(cache.get(&"counter".to_string()), Some(15));

        let missing = cache.update(&"non_existent".to_string(), |val| {
            *val += 1;
        });
        assert!(!missing);
    }

    #[test]
    fn test_ttl_cache_get_or_insert_with() {
        let cache: TtlCache<String, String> = TtlCache::with_ttl_secs(60, 100);
        let val1 = cache.get_or_insert_with("user_123".to_string(), || "computed_value".to_string());
        assert_eq!(val1, "computed_value");

        // Second call should return cached value without invoking generator
        let val2 = cache.get_or_insert_with("user_123".to_string(), || "should_not_run".to_string());
        assert_eq!(val2, "computed_value");
    }

    #[test]
    fn test_ttl_cache_clear_and_remove() {
        let cache: TtlCache<String, i32> = TtlCache::with_ttl_secs(60, 100);
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);

        assert_eq!(cache.remove(&"a".to_string()), Some(1));
        assert_eq!(cache.get(&"a".to_string()), None);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
