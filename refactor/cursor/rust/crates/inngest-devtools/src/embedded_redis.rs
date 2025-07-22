//! Embedded Redis implementation for development
//!
//! This module provides a Redis-compatible in-memory database similar to miniredis in Go.

use dashmap::DashMap;
use inngest_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio::time::interval;

/// Configuration for embedded Redis
#[derive(Debug, Clone)]
pub struct EmbeddedRedisConfig {
    /// Whether to persist data to disk
    pub persist: bool,
    /// Time tick interval in milliseconds (for time simulation)
    pub tick_interval_ms: u64,
    /// Data directory for persistence
    pub data_dir: Option<String>,
}

impl Default for EmbeddedRedisConfig {
    fn default() -> Self {
        Self {
            persist: false,
            tick_interval_ms: 150,
            data_dir: None,
        }
    }
}

/// Redis value types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedisValue {
    String(String),
    Hash(HashMap<String, String>),
    List(VecDeque<String>),
    Set(std::collections::HashSet<String>),
    ZSet(Vec<(f64, String)>), // (score, member)
}

impl RedisValue {
    /// Get as string (for simple string values)
    pub fn as_string(&self) -> Option<&str> {
        if let RedisValue::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Get as hash
    pub fn as_hash(&self) -> Option<&HashMap<String, String>> {
        if let RedisValue::Hash(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Get as list
    pub fn as_list(&self) -> Option<&VecDeque<String>> {
        if let RedisValue::List(l) = self {
            Some(l)
        } else {
            None
        }
    }
}

/// Key expiration information
#[derive(Debug, Clone)]
struct KeyExpiry {
    /// Expiration timestamp (Unix timestamp in milliseconds)
    expire_at: u64,
}

/// Embedded Redis server instance
pub struct EmbeddedRedis {
    /// Key-value store
    data: Arc<DashMap<String, RedisValue>>,
    /// Key expiration tracking
    expiry: Arc<DashMap<String, KeyExpiry>>,
    /// Current virtual time (Unix timestamp in milliseconds)
    current_time: Arc<AtomicU64>,
    /// Configuration
    config: EmbeddedRedisConfig,
    /// Shutdown signal
    shutdown: Arc<Notify>,
}

impl EmbeddedRedis {
    /// Create a new embedded Redis instance
    pub async fn new(config: EmbeddedRedisConfig) -> Result<Self> {
        let redis = Self {
            data: Arc::new(DashMap::new()),
            expiry: Arc::new(DashMap::new()),
            current_time: Arc::new(AtomicU64::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )),
            config: config.clone(),
            shutdown: Arc::new(Notify::new()),
        };

        // Start time tick simulation
        if config.tick_interval_ms > 0 {
            redis.start_time_ticker().await;
        }

        Ok(redis)
    }

    /// Start the time ticker for simulating time progression
    async fn start_time_ticker(&self) {
        let current_time = self.current_time.clone();
        let tick_interval = self.config.tick_interval_ms;
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(tick_interval));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        current_time.fetch_add(tick_interval, Ordering::Relaxed);
                    }
                    _ = shutdown.notified() => {
                        break;
                    }
                }
            }
        });
    }

    /// Get current virtual time
    pub fn current_time(&self) -> u64 {
        self.current_time.load(Ordering::Relaxed)
    }

    /// Fast forward time by the specified duration
    pub fn fast_forward(&self, duration: Duration) {
        let ms = duration.as_millis() as u64;
        self.current_time.fetch_add(ms, Ordering::Relaxed);
    }

    /// Clean up expired keys
    fn cleanup_expired_keys(&self) {
        let now = self.current_time();
        let mut expired_keys = Vec::new();

        // Find expired keys
        for entry in self.expiry.iter() {
            if entry.value().expire_at <= now {
                expired_keys.push(entry.key().clone());
            }
        }

        // Remove expired keys
        for key in expired_keys {
            self.expiry.remove(&key);
            self.data.remove(&key);
        }
    }

    /// Check if a key exists and is not expired
    pub fn exists(&self, key: &str) -> bool {
        self.cleanup_expired_keys();
        self.data.contains_key(key)
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Option<RedisValue> {
        self.cleanup_expired_keys();
        self.data.get(key).map(|entry| entry.value().clone())
    }

    /// Set a string value
    pub fn set(&self, key: &str, value: &str) {
        self.data
            .insert(key.to_string(), RedisValue::String(value.to_string()));
    }

    /// Set a string value with expiration
    pub fn setex(&self, key: &str, value: &str, ttl_seconds: u64) {
        self.set(key, value);
        let expire_at = self.current_time() + (ttl_seconds * 1000);
        self.expiry.insert(key.to_string(), KeyExpiry { expire_at });
    }

    /// Delete a key
    pub fn del(&self, key: &str) -> bool {
        let removed_data = self.data.remove(key).is_some();
        self.expiry.remove(key);
        removed_data
    }

    /// Hash operations
    pub fn hget(&self, key: &str, field: &str) -> Option<String> {
        self.cleanup_expired_keys();
        self.data
            .get(key)
            .and_then(|entry| entry.value().as_hash().and_then(|h| h.get(field).cloned()))
    }

    pub fn hset(&self, key: &str, field: &str, value: &str) {
        let mut hash = self
            .data
            .get(key)
            .and_then(|entry| entry.value().as_hash().cloned())
            .unwrap_or_default();

        hash.insert(field.to_string(), value.to_string());
        self.data.insert(key.to_string(), RedisValue::Hash(hash));
    }

    pub fn hgetall(&self, key: &str) -> HashMap<String, String> {
        self.cleanup_expired_keys();
        self.data
            .get(key)
            .and_then(|entry| entry.value().as_hash().cloned())
            .unwrap_or_default()
    }

    /// List operations
    pub fn lpush(&self, key: &str, value: &str) {
        let mut list = self
            .data
            .get(key)
            .and_then(|entry| entry.value().as_list().cloned())
            .unwrap_or_default();

        list.push_front(value.to_string());
        self.data.insert(key.to_string(), RedisValue::List(list));
    }

    pub fn rpush(&self, key: &str, value: &str) {
        let mut list = self
            .data
            .get(key)
            .and_then(|entry| entry.value().as_list().cloned())
            .unwrap_or_default();

        list.push_back(value.to_string());
        self.data.insert(key.to_string(), RedisValue::List(list));
    }

    pub async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let list = self
            .data
            .get(key)
            .and_then(|entry| entry.value().as_list().cloned())
            .unwrap_or_default();

        let start = if start < 0 {
            (list.len() as isize + start).max(0) as usize
        } else {
            start.min(list.len() as isize) as usize
        };

        let stop = if stop < 0 {
            (list.len() as isize + stop).max(0) as usize
        } else {
            (stop as usize).min(list.len())
        };

        if start >= stop {
            return Ok(vec![]);
        }

        Ok(list.range(start..stop).cloned().collect())
    }

    /// Flush all data
    pub async fn flush_all(&self) -> Result<()> {
        self.data.clear();
        self.expiry.clear();
        Ok(())
    }

    /// Export all data as a snapshot
    pub async fn export_data(&self) -> Result<RedisSnapshot> {
        self.cleanup_expired_keys();

        let mut data = HashMap::new();
        for entry in self.data.iter() {
            data.insert(entry.key().clone(), entry.value().clone());
        }

        let mut expiry = HashMap::new();
        for entry in self.expiry.iter() {
            expiry.insert(entry.key().clone(), entry.value().expire_at);
        }

        Ok(RedisSnapshot {
            current_time: self.current_time(),
            data,
            expiry,
        })
    }

    /// Import data from a snapshot
    pub async fn import_data(&self, snapshot: RedisSnapshot) -> Result<()> {
        self.flush_all().await?;

        // Restore current time
        self.current_time
            .store(snapshot.current_time, Ordering::Relaxed);

        // Restore data
        for (key, value) in snapshot.data {
            self.data.insert(key, value);
        }

        // Restore expiry
        for (key, expire_at) in snapshot.expiry {
            self.expiry.insert(key, KeyExpiry { expire_at });
        }

        Ok(())
    }

    /// Get Redis-compatible connection string
    pub fn connection_string(&self) -> String {
        // Since this is embedded, we'll return a special embedded:// scheme
        "embedded://localhost:0".to_string()
    }
}

impl Drop for EmbeddedRedis {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}

/// Snapshot of Redis data for persistence/testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedisSnapshot {
    pub current_time: u64,
    pub data: HashMap<String, RedisValue>,
    pub expiry: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_embedded_redis_basic_operations() {
        let config = EmbeddedRedisConfig::default();
        let redis = EmbeddedRedis::new(config).await.unwrap();

        // Test string operations
        redis.set("key1", "value1");
        assert_eq!(redis.get("key1").unwrap().as_string(), Some("value1"));
        assert!(redis.exists("key1"));

        // Test expiration
        redis.setex("key2", "value2", 1);
        assert!(redis.exists("key2"));
        redis.fast_forward(Duration::from_secs(2));
        assert!(!redis.exists("key2"));

        // Test hash operations
        redis.hset("hash1", "field1", "value1");
        redis.hset("hash1", "field2", "value2");
        assert_eq!(redis.hget("hash1", "field1"), Some("value1".to_string()));
        let hash = redis.hgetall("hash1");
        assert_eq!(hash.len(), 2);

        // Test list operations
        redis.lpush("list1", "item1");
        redis.rpush("list1", "item2");
        let list = redis.lrange("list1", 0, -1).await.unwrap();
        assert_eq!(list, vec!["item1", "item2"]);

        // Test deletion
        assert!(redis.del("key1"));
        assert!(!redis.exists("key1"));
    }

    #[tokio::test]
    async fn test_embedded_redis_snapshot() {
        let config = EmbeddedRedisConfig::default();
        let redis = EmbeddedRedis::new(config).await.unwrap();

        // Add some data
        redis.set("key1", "value1");
        redis.hset("hash1", "field1", "value1");

        // Export snapshot
        let snapshot = redis.export_data().await.unwrap();
        assert!(snapshot.data.contains_key("key1"));
        assert!(snapshot.data.contains_key("hash1"));

        // Clear data
        redis.flush_all().await.unwrap();
        assert!(!redis.exists("key1"));

        // Import snapshot
        redis.import_data(snapshot).await.unwrap();
        assert!(redis.exists("key1"));
        assert!(redis.exists("hash1"));
        assert_eq!(redis.get("key1").unwrap().as_string(), Some("value1"));
    }

    #[tokio::test]
    async fn test_time_simulation() {
        let config = EmbeddedRedisConfig {
            tick_interval_ms: 10,
            ..Default::default()
        };
        let redis = EmbeddedRedis::new(config).await.unwrap();

        let initial_time = redis.current_time();
        redis.fast_forward(Duration::from_secs(5));
        let after_time = redis.current_time();

        assert!(after_time >= initial_time + 5000);
    }
}
