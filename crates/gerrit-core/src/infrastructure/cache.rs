// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! In-memory cache with per-entry TTL and LRU eviction.
//!
//! Uses [`lru::LruCache`] wrapped in a [`std::sync::Mutex`] for
//! concurrent access. Entries are lazy-evicted: stale entries are
//! detected on access and removed. When capacity is exceeded, the
//! least-recently-used entry is evicted. Uses [`tokio::time::Instant`]
//! for deterministic time in tests.

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;

use lru::LruCache;
use tokio::time::Instant;

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/// A concurrent in-memory cache with per-entry time-to-live (TTL)
/// and LRU eviction.
///
/// Entries are stored with their insertion time and evicted lazily
/// when accessed after expiration. When the cache is at capacity,
/// new inserts evict the least-recently-used entry.
#[derive(Debug)]
pub struct MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    inner: Mutex<LruCache<K, CacheEntry<V>>>,
    ttl: Duration,
    max_entries: usize,
}

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
}

impl<V> CacheEntry<V> {
    fn new(value: V) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: &Duration) -> bool {
        self.inserted_at.elapsed() >= *ttl
    }
}

impl<K, V> Clone for MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    fn clone(&self) -> Self {
        let inner = self.inner.lock().unwrap();
        let cloned_inner = Mutex::new(inner.clone());
        Self {
            inner: cloned_inner,
            ttl: self.ttl,
            max_entries: self.max_entries,
        }
    }
}

impl<K, V> MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    /// Creates a new cache with the given TTL and maximum entry count.
    #[must_use]
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        let cap = NonZeroUsize::new(max_entries).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            inner: Mutex::new(LruCache::new(cap)),
            ttl,
            max_entries,
        }
    }

    /// Inserts a value into the cache. If the cache is at capacity,
    /// the least-recently-used entry is evicted automatically by LruCache.
    pub fn insert(&self, key: K, value: V) {
        let mut cache = self.inner.lock().unwrap();
        cache.put(key, CacheEntry::new(value));
    }

    /// Retrieves a cached value, returning `None` if the key is not
    /// present or the entry has expired.
    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.inner.lock().unwrap();

        let is_expired = cache
            .peek(key)
            .map(|e| e.is_expired(&self.ttl))
            .unwrap_or(false);

        if is_expired {
            cache.pop(key);
            return None;
        }

        // get() promotes the entry to most-recently-used
        cache.get(key).map(|e| e.value.clone())
    }

    /// Removes a key from the cache.
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut cache = self.inner.lock().unwrap();
        cache.pop(key).map(|e| e.value)
    }

    /// Returns the number of entries currently in the cache (including
    /// potentially stale ones that haven't been evicted yet).
    #[must_use]
    pub fn len(&self) -> usize {
        let cache = self.inner.lock().unwrap();
        cache.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let cache = self.inner.lock().unwrap();
        cache.is_empty()
    }

    /// Evicts all expired entries.
    pub fn evict_expired(&self) {
        let mut cache = self.inner.lock().unwrap();
        let ttl = self.ttl;
        let expired_keys: Vec<K> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired(&ttl))
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired_keys {
            cache.pop(&key);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    #[test]
    fn insert_and_retrieve() {
        let cache = MemoryCache::<String, String>::new(Duration::from_secs(60), 100);
        cache.insert("key".into(), "value".into());
        assert_eq!(cache.get(&"key".into()), Some("value".into()));
    }

    #[test]
    fn missing_key_returns_none() {
        let cache = MemoryCache::<String, String>::new(Duration::from_secs(60), 100);
        assert_eq!(cache.get(&"missing".into()), None);
    }

    #[test]
    fn remove_returns_value() {
        let cache = MemoryCache::<String, i32>::new(Duration::from_secs(60), 100);
        cache.insert("a".into(), 42);
        assert_eq!(cache.remove(&"a".into()), Some(42));
        assert_eq!(cache.get(&"a".into()), None);
    }

    #[test]
    fn len_reflects_inserts() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_secs(60), 100);
        assert_eq!(cache.len(), 0);
        cache.insert(1, 10);
        assert_eq!(cache.len(), 1);
        cache.insert(2, 20);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn is_empty() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_secs(60), 100);
        assert!(cache.is_empty());
        cache.insert(1, 10);
        assert!(!cache.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn entry_expires_after_ttl() {
        let cache = MemoryCache::<String, String>::new(Duration::from_millis(10), 100);
        cache.insert("key".into(), "value".into());

        assert!(cache.get(&"key".into()).is_some());

        tokio::time::advance(Duration::from_millis(15)).await;

        assert!(cache.get(&"key".into()).is_none());
        assert!(cache.is_empty(), "expired entry should be evicted");
    }

    #[tokio::test(start_paused = true)]
    async fn evict_expired_removes_stale_entries() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_millis(10), 100);
        cache.insert(1, 10);
        cache.insert(2, 20);

        tokio::time::advance(Duration::from_millis(15)).await;

        cache.evict_expired();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn capacity_limit_triggers_lru_eviction() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_secs(3600), 10);

        for i in 0..20 {
            cache.insert(i, i * 10);
        }

        // After inserting 20 entries with max_entries=10, we should have exactly 10
        assert_eq!(cache.len(), 10, "expected 10, got {}", cache.len());

        // The first entries should be evicted (keys 0-9), keys 10-19 remain
        for i in 0..10 {
            assert!(
                cache.get(&i).is_none(),
                "key {i} should have been evicted by LRU"
            );
        }
        for i in 10..20 {
            assert_eq!(
                cache.get(&i),
                Some(i * 10),
                "key {i} should still be present"
            );
        }
    }

    #[test]
    fn lru_access_promotes_entry() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_secs(3600), 3);

        cache.insert(1, 10);
        cache.insert(2, 20);
        cache.insert(3, 30);

        // Access key 1 to promote it to MRU
        assert_eq!(cache.get(&1), Some(10));

        // Insert a new key that evicts the LRU (key 2)
        cache.insert(4, 40);

        // Key 2 should be evicted (least recently used — never accessed)
        assert!(cache.get(&2).is_none());
        // Keys 1, 3, 4 remain
        assert_eq!(cache.get(&1), Some(10));
        assert_eq!(cache.get(&3), Some(30));
        assert_eq!(cache.get(&4), Some(40));
    }

    #[test]
    fn cache_clone_preserves_entries() {
        let cache = MemoryCache::<i32, i32>::new(Duration::from_secs(60), 100);
        cache.insert(1, 10);
        cache.insert(2, 20);

        let clone = cache.clone();
        assert_eq!(clone.get(&1), Some(10));
        assert_eq!(clone.get(&2), Some(20));
    }
}
