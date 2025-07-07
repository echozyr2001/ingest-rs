//! State caching for performance optimization
//!
//! This module provides in-memory caching of frequently accessed states
//! to improve performance and reduce database load.

use crate::ExecutionState;
use ingest_core::Id;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// In-memory state cache with TTL and LRU eviction
pub struct StateCache {
    /// Cache storage
    cache: Arc<RwLock<HashMap<Id, CacheEntry>>>,
    /// Maximum cache size
    max_size: usize,
    /// Default TTL for cache entries
    default_ttl: Duration,
    /// Access order for LRU eviction
    access_order: Arc<RwLock<Vec<Id>>>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    state: ExecutionState,
    inserted_at: Instant,
    last_accessed: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(state: ExecutionState, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            state,
            inserted_at: now,
            last_accessed: now,
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

impl StateCache {
    /// Create a new state cache
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            default_ttl: Duration::from_secs(300), // 5 minutes default TTL
            access_order: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new state cache with custom TTL
    pub fn with_ttl(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            default_ttl,
            access_order: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Put a state in the cache
    pub async fn put(&self, run_id: Id, state: ExecutionState) {
        self.put_with_ttl(run_id, state, self.default_ttl).await;
    }

    /// Put a state in the cache with custom TTL
    pub async fn put_with_ttl(&self, run_id: Id, state: ExecutionState, ttl: Duration) {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        // Check if we need to evict entries
        if cache.len() >= self.max_size && !cache.contains_key(&run_id) {
            self.evict_lru(&mut cache, &mut access_order).await;
        }

        // Insert or update the entry
        let entry = CacheEntry::new(state, ttl);
        cache.insert(run_id.clone(), entry);

        // Update access order
        if let Some(pos) = access_order.iter().position(|id| id == &run_id) {
            access_order.remove(pos);
        }
        access_order.push(run_id);
    }

    /// Get a state from the cache
    pub async fn get(&self, run_id: &Id) -> Option<ExecutionState> {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        if let Some(entry) = cache.get_mut(run_id) {
            // Check if entry is expired
            if entry.is_expired() {
                cache.remove(run_id);
                if let Some(pos) = access_order.iter().position(|id| id == run_id) {
                    access_order.remove(pos);
                }
                return None;
            }

            // Touch the entry and update access order
            entry.touch();
            if let Some(pos) = access_order.iter().position(|id| id == run_id) {
                access_order.remove(pos);
                access_order.push(run_id.clone());
            }

            Some(entry.state.clone())
        } else {
            None
        }
    }

    /// Remove a state from the cache
    pub async fn remove(&self, run_id: &Id) {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        cache.remove(run_id);
        if let Some(pos) = access_order.iter().position(|id| id == run_id) {
            access_order.remove(pos);
        }
    }

    /// Check if a state is in the cache
    pub async fn contains(&self, run_id: &Id) -> bool {
        let cache = self.cache.read().await;

        if let Some(entry) = cache.get(run_id) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Clear all expired entries
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        let initial_size = cache.len();

        // Collect expired keys
        let expired_keys: Vec<Id> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        // Remove expired entries
        for key in &expired_keys {
            cache.remove(key);
            if let Some(pos) = access_order.iter().position(|id| id == key) {
                access_order.remove(pos);
            }
        }

        initial_size - cache.len()
    }

    /// Clear all entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        cache.clear();
        access_order.clear();
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let access_order = self.access_order.read().await;

        let total_entries = cache.len();
        let expired_entries = cache.values().filter(|entry| entry.is_expired()).count();
        let active_entries = total_entries - expired_entries;

        let memory_usage = std::mem::size_of::<HashMap<Id, CacheEntry>>()
            + cache.capacity() * (std::mem::size_of::<Id>() + std::mem::size_of::<CacheEntry>())
            + access_order.capacity() * std::mem::size_of::<Id>();

        CacheStats {
            total_entries,
            active_entries,
            expired_entries,
            max_size: self.max_size,
            memory_usage_bytes: memory_usage,
            hit_rate: 0.0, // Would need to track hits/misses for this
        }
    }

    /// Evict least recently used entry
    async fn evict_lru(&self, cache: &mut HashMap<Id, CacheEntry>, access_order: &mut Vec<Id>) {
        if let Some(lru_key) = access_order.first().cloned() {
            cache.remove(&lru_key);
            access_order.remove(0);
        }
    }

    /// Get all cached run IDs (for debugging)
    pub async fn get_cached_ids(&self) -> Vec<Id> {
        let cache = self.cache.read().await;
        cache.keys().cloned().collect()
    }

    /// Force eviction of specific entries
    pub async fn evict_by_function(&self, function_id: &Id) {
        let mut cache = self.cache.write().await;
        let mut access_order = self.access_order.write().await;

        // Find entries to evict
        let to_evict: Vec<Id> = cache
            .iter()
            .filter(|(_, entry)| &entry.state.function_id == function_id)
            .map(|(key, _)| key.clone())
            .collect();

        // Remove them
        for key in &to_evict {
            cache.remove(key);
            if let Some(pos) = access_order.iter().position(|id| id == key) {
                access_order.remove(pos);
            }
        }
    }

    /// Warm up cache with frequently accessed states
    pub async fn warmup(&self, states: Vec<ExecutionState>) {
        for state in states {
            self.put(state.run_id.clone(), state).await;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
    pub max_size: usize,
    pub memory_usage_bytes: usize,
    pub hit_rate: f64,
}

impl Default for StateCache {
    fn default() -> Self {
        Self::new(1000) // Default cache size of 1000 entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExecutionContext, ExecutionStatus};
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = StateCache::new(10);
        let run_id = generate_id_with_prefix("run");

        let state = ExecutionState {
            run_id: run_id.clone(),
            function_id: generate_id_with_prefix("fn"),
            status: ExecutionStatus::Running,
            current_step: None,
            variables: HashMap::new(),
            context: ExecutionContext::default(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            scheduled_at: None,
            completed_at: None,
        };

        // Should not be in cache initially
        assert!(!cache.contains(&run_id).await);
        assert!(cache.get(&run_id).await.is_none());

        // Put in cache
        cache.put(run_id.clone(), state.clone()).await;

        // Should be in cache now
        assert!(cache.contains(&run_id).await);
        let cached_state = cache.get(&run_id).await.unwrap();
        assert_eq!(cached_state.run_id, state.run_id);
        assert_eq!(cached_state.status, state.status);

        // Remove from cache
        cache.remove(&run_id).await;

        // Should not be in cache anymore
        assert!(!cache.contains(&run_id).await);
        assert!(cache.get(&run_id).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let cache = StateCache::with_ttl(10, Duration::from_millis(100));
        let run_id = generate_id_with_prefix("run");

        let state = ExecutionState {
            run_id: run_id.clone(),
            function_id: generate_id_with_prefix("fn"),
            status: ExecutionStatus::Running,
            current_step: None,
            variables: HashMap::new(),
            context: ExecutionContext::default(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            scheduled_at: None,
            completed_at: None,
        };

        // Put in cache
        cache.put(run_id.clone(), state).await;

        // Should be in cache initially
        assert!(cache.contains(&run_id).await);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should be expired now
        assert!(!cache.contains(&run_id).await);
        assert!(cache.get(&run_id).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let cache = StateCache::new(2); // Small cache size

        // Create test states
        let states: Vec<ExecutionState> = (0..3)
            .map(|i| ExecutionState {
                run_id: generate_id_with_prefix(&format!("run_{}", i)),
                function_id: generate_id_with_prefix("fn"),
                status: ExecutionStatus::Running,
                current_step: None,
                variables: HashMap::new(),
                context: ExecutionContext::default(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                scheduled_at: None,
                completed_at: None,
            })
            .collect();

        // Put first two states
        cache.put(states[0].run_id.clone(), states[0].clone()).await;
        cache.put(states[1].run_id.clone(), states[1].clone()).await;

        // Both should be in cache
        assert!(cache.contains(&states[0].run_id).await);
        assert!(cache.contains(&states[1].run_id).await);

        // Put third state (should evict first one)
        cache.put(states[2].run_id.clone(), states[2].clone()).await;

        // First should be evicted, second and third should remain
        assert!(!cache.contains(&states[0].run_id).await);
        assert!(cache.contains(&states[1].run_id).await);
        assert!(cache.contains(&states[2].run_id).await);
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let cache = StateCache::with_ttl(10, Duration::from_millis(100));

        // Add multiple states
        for i in 0..5 {
            let state = ExecutionState {
                run_id: generate_id_with_prefix(&format!("run_{}", i)),
                function_id: generate_id_with_prefix("fn"),
                status: ExecutionStatus::Running,
                current_step: None,
                variables: HashMap::new(),
                context: ExecutionContext::default(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                scheduled_at: None,
                completed_at: None,
            };
            cache.put(state.run_id.clone(), state).await;
        }

        // All should be active
        let stats = cache.get_stats().await;
        assert_eq!(stats.active_entries, 5);
        assert_eq!(stats.expired_entries, 0);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Clean up expired entries
        let cleaned = cache.cleanup_expired().await;
        assert_eq!(cleaned, 5);

        // Should be empty now
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = StateCache::new(10);

        // Initially empty
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.active_entries, 0);
        assert_eq!(stats.max_size, 10);

        // Add some states
        for i in 0..3 {
            let state = ExecutionState {
                run_id: generate_id_with_prefix(&format!("run_{}", i)),
                function_id: generate_id_with_prefix("fn"),
                status: ExecutionStatus::Running,
                current_step: None,
                variables: HashMap::new(),
                context: ExecutionContext::default(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                scheduled_at: None,
                completed_at: None,
            };
            cache.put(state.run_id.clone(), state).await;
        }

        // Should have 3 active entries
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.active_entries, 3);
        assert_eq!(stats.expired_entries, 0);
    }
}
