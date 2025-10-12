//! Caching utilities for ryt

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Simple in-memory cache with TTL
#[derive(Clone)]
pub struct MemoryCache<K, V> {
    cache: Arc<Mutex<HashMap<K, CachedValue<V>>>>,
}

#[derive(Clone)]
struct CachedValue<V> {
    value: V,
    expires_at: Instant,
}

impl<K, V> MemoryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(cached_value) = cache.get(key) {
            if cached_value.expires_at > Instant::now() {
                return Some(cached_value.value.clone());
            } else {
                cache.remove(key);
            }
        }
        None
    }

    pub fn insert(&self, key: K, value: V, ttl: Duration) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(
            key,
            CachedValue {
                value,
                expires_at: Instant::now() + ttl,
            },
        );
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(key).map(|cached_value| cached_value.value)
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();
        cache.retain(|_, cached_value| cached_value.expires_at > now);
    }
}

impl<K, V> Default for MemoryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// High-performance async cache using moka
pub type AsyncCache<K, V> = Cache<K, V>;

/// Create a new async cache with TTL
pub fn new_async_cache<K, V>(ttl: Duration) -> AsyncCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    Cache::builder().time_to_live(ttl).build()
}

/// Create a new async cache with TTL and max capacity
pub fn new_async_cache_with_capacity<K, V>(ttl: Duration, max_capacity: u64) -> AsyncCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    Cache::builder()
        .time_to_live(ttl)
        .max_capacity(max_capacity)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_memory_cache() {
        let cache = MemoryCache::new();

        // Test insert and get
        cache.insert("key1", "value1", Duration::from_secs(1));
        assert_eq!(cache.get(&"key1"), Some("value1"));

        // Test expiration
        thread::sleep(Duration::from_millis(1100));
        assert_eq!(cache.get(&"key1"), None);

        // Test remove
        cache.insert("key2", "value2", Duration::from_secs(10));
        assert_eq!(cache.remove(&"key2"), Some("value2"));
        assert_eq!(cache.get(&"key2"), None);

        // Test clear
        cache.insert("key3", "value3", Duration::from_secs(10));
        cache.clear();
        assert_eq!(cache.get(&"key3"), None);
    }

    #[test]
    fn test_cleanup_expired() {
        let cache = MemoryCache::new();

        cache.insert("key1", "value1", Duration::from_millis(100));
        cache.insert("key2", "value2", Duration::from_secs(10));

        thread::sleep(Duration::from_millis(150));
        cache.cleanup_expired();

        assert_eq!(cache.get(&"key1"), None);
        assert_eq!(cache.get(&"key2"), Some("value2"));
    }

    #[tokio::test]
    async fn test_async_cache() {
        let cache = new_async_cache(Duration::from_secs(1));

        cache.insert("key1", "value1").await;
        assert_eq!(cache.get(&"key1").await, Some("value1"));

        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(cache.get(&"key1").await, None);
    }
}

/// Multi-level cache for YouTube data
#[derive(Clone)]
pub struct MultiLevelCache {
    /// Player.js cache (10 minutes)
    player_js_cache: Arc<Cache<String, String>>,
    /// Signature cache (1 hour)
    signature_cache: Arc<Cache<String, String>>,
    /// Visitor ID cache (10 hours)
    visitor_id_cache: Arc<Cache<String, String>>,
    /// Botguard token cache (30 minutes)
    botguard_cache: Arc<Cache<String, String>>,
}

impl MultiLevelCache {
    /// Create a new multi-level cache
    pub fn new() -> Self {
        Self {
            player_js_cache: Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(600)) // 10 minutes
                    .build(),
            ),
            signature_cache: Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(3600)) // 1 hour
                    .build(),
            ),
            visitor_id_cache: Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(36000)) // 10 hours
                    .build(),
            ),
            botguard_cache: Arc::new(
                Cache::builder()
                    .time_to_live(Duration::from_secs(1800)) // 30 minutes
                    .build(),
            ),
        }
    }

    /// Get player.js content
    pub async fn get_player_js(&self, url: &str) -> Option<String> {
        self.player_js_cache.get(url).await
    }

    /// Set player.js content
    pub async fn set_player_js(&self, url: &str, content: String) {
        self.player_js_cache.insert(url.to_string(), content).await;
    }

    /// Get signature
    pub async fn get_signature(&self, signature: &str) -> Option<String> {
        self.signature_cache.get(signature).await
    }

    /// Set signature
    pub async fn set_signature(&self, signature: &str, deciphered: String) {
        self.signature_cache
            .insert(signature.to_string(), deciphered)
            .await;
    }

    /// Get visitor ID
    pub async fn get_visitor_id(&self, key: &str) -> Option<String> {
        self.visitor_id_cache.get(key).await
    }

    /// Set visitor ID
    pub async fn set_visitor_id(&self, key: &str, visitor_id: String) {
        self.visitor_id_cache
            .insert(key.to_string(), visitor_id)
            .await;
    }

    /// Get botguard token
    pub async fn get_botguard_token(&self, key: &str) -> Option<String> {
        self.botguard_cache.get(key).await
    }

    /// Set botguard token
    pub async fn set_botguard_token(&self, key: &str, token: String) {
        self.botguard_cache.insert(key.to_string(), token).await;
    }

    /// Clear all caches
    pub async fn clear_all(&self) {
        self.player_js_cache.invalidate_all();
        self.signature_cache.invalidate_all();
        self.visitor_id_cache.invalidate_all();
        self.botguard_cache.invalidate_all();
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        CacheStats {
            player_js_entries: self.player_js_cache.entry_count(),
            signature_entries: self.signature_cache.entry_count(),
            visitor_id_entries: self.visitor_id_cache.entry_count(),
            botguard_entries: self.botguard_cache.entry_count(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub player_js_entries: u64,
    pub signature_entries: u64,
    pub visitor_id_entries: u64,
    pub botguard_entries: u64,
}

impl Default for MultiLevelCache {
    fn default() -> Self {
        Self::new()
    }
}
