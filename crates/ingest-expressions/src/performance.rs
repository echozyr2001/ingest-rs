//! Performance optimization and caching for expression evaluation

use crate::parser::ParsedExpression;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

/// Performance configuration for expression evaluation
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Cache size for parsed expressions
    cache_size: usize,
    /// Enable expression compilation
    compilation_enabled: bool,
    /// Cache TTL (time to live)
    cache_ttl: Option<Duration>,
    /// Enable performance metrics collection
    metrics_enabled: bool,
    /// Maximum cache memory usage in bytes
    max_cache_memory: usize,
}

impl PerformanceConfig {
    /// Create a new performance configuration with default values
    pub fn new() -> Self {
        Self {
            cache_size: 1000,
            compilation_enabled: true,
            cache_ttl: Some(Duration::from_secs(300)), // 5 minutes
            metrics_enabled: true,
            max_cache_memory: 10 * 1024 * 1024, // 10MB
        }
    }

    /// Set cache size
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }

    /// Enable or disable compilation
    pub fn with_compilation_enabled(mut self, enabled: bool) -> Self {
        self.compilation_enabled = enabled;
        self
    }

    /// Set cache TTL
    pub fn with_cache_ttl(mut self, ttl: Option<Duration>) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Enable or disable metrics collection
    pub fn with_metrics_enabled(mut self, enabled: bool) -> Self {
        self.metrics_enabled = enabled;
        self
    }

    /// Set maximum cache memory usage
    pub fn with_max_cache_memory(mut self, bytes: usize) -> Self {
        self.max_cache_memory = bytes;
        self
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache_size
    }

    /// Check if compilation is enabled
    pub fn is_compilation_enabled(&self) -> bool {
        self.compilation_enabled
    }

    /// Get cache TTL
    pub fn cache_ttl(&self) -> Option<Duration> {
        self.cache_ttl
    }

    /// Check if metrics are enabled
    pub fn is_metrics_enabled(&self) -> bool {
        self.metrics_enabled
    }

    /// Get maximum cache memory usage
    pub fn max_cache_memory(&self) -> usize {
        self.max_cache_memory
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache entry for parsed expressions
#[derive(Debug)]
struct CacheEntry {
    /// The parsed expression
    expression: ParsedExpression,
    /// When this entry was created
    created_at: Instant,
    /// Number of times this entry has been accessed
    access_count: u64,
    /// Last access time
    last_accessed: Instant,
    /// Estimated memory usage of this entry
    memory_usage: usize,
}

impl CacheEntry {
    fn new(expression: ParsedExpression) -> Self {
        let now = Instant::now();
        let memory_usage = std::mem::size_of_val(&expression) + expression.source().len();

        Self {
            expression,
            created_at: now,
            access_count: 0,
            last_accessed: now,
            memory_usage,
        }
    }

    fn access(&mut self) -> &ParsedExpression {
        self.access_count += 1;
        self.last_accessed = Instant::now();
        &self.expression
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    fn memory_usage(&self) -> usize {
        self.memory_usage
    }
}

// Safety: CacheEntry is Send and Sync because all its fields are Send and Sync
unsafe impl Send for CacheEntry {}
unsafe impl Sync for CacheEntry {}

/// Expression cache for performance optimization
#[derive(Debug)]
pub struct ExpressionCache {
    /// Cache storage
    cache: DashMap<String, Arc<Mutex<CacheEntry>>>,
    /// Configuration
    config: PerformanceConfig,
    /// Performance statistics
    stats: Arc<RwLock<CacheStats>>,
}

impl ExpressionCache {
    /// Create a new expression cache
    pub fn new(cache_size: usize) -> Self {
        Self {
            cache: DashMap::new(),
            config: PerformanceConfig::new().with_cache_size(cache_size),
            stats: Arc::new(RwLock::new(CacheStats::new())),
        }
    }

    /// Create a new expression cache with configuration
    pub fn with_config(config: PerformanceConfig) -> Self {
        Self {
            cache: DashMap::new(),
            config,
            stats: Arc::new(RwLock::new(CacheStats::new())),
        }
    }

    /// Get a cached expression
    pub fn get(&self, key: &str) -> Option<ParsedExpression> {
        if let Some(entry_ref) = self.cache.get(key) {
            let mut entry = entry_ref.value().lock().unwrap();

            // Check if entry is expired
            if let Some(ttl) = self.config.cache_ttl {
                if entry.is_expired(ttl) {
                    drop(entry);
                    self.cache.remove(key);
                    self.stats.write().unwrap().cache_misses += 1;
                    return None;
                }
            }

            let expression = entry.access().clone();
            self.stats.write().unwrap().cache_hits += 1;
            Some(expression)
        } else {
            self.stats.write().unwrap().cache_misses += 1;
            None
        }
    }

    /// Insert an expression into the cache
    pub fn insert(&self, key: String, expression: ParsedExpression) {
        // Check if we need to evict entries
        self.maybe_evict();

        let entry = Arc::new(Mutex::new(CacheEntry::new(expression)));
        self.cache.insert(key, entry);
        self.stats.write().unwrap().cache_inserts += 1;
    }

    /// Remove an expression from the cache
    pub fn remove(&self, key: &str) -> Option<ParsedExpression> {
        self.cache.remove(key).map(|(_, entry)| {
            self.stats.write().unwrap().cache_evictions += 1;
            entry.lock().unwrap().expression.clone()
        })
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        let count = self.cache.len();
        self.cache.clear();
        self.stats.write().unwrap().cache_evictions += count as u64;
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.cache
            .iter()
            .map(|entry| entry.value().lock().unwrap().memory_usage())
            .sum()
    }

    /// Maybe evict entries based on cache size and memory limits
    fn maybe_evict(&self) {
        // Check size limit
        if self.cache.len() >= self.config.cache_size {
            self.evict_lru();
        }

        // Check memory limit
        if self.memory_usage() > self.config.max_cache_memory {
            self.evict_by_memory();
        }

        // Check TTL
        if let Some(ttl) = self.config.cache_ttl {
            self.evict_expired(ttl);
        }
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        let mut oldest_key: Option<String> = None;
        let mut oldest_time = Instant::now();

        for entry in self.cache.iter() {
            let last_accessed = entry.value().lock().unwrap().last_accessed;
            if last_accessed < oldest_time {
                oldest_time = last_accessed;
                oldest_key = Some(entry.key().clone());
            }
        }

        if let Some(key) = oldest_key {
            self.remove(&key);
        }
    }

    /// Evict entries to free memory
    fn evict_by_memory(&self) {
        // Sort entries by access count (ascending) and remove least accessed
        let mut entries: Vec<_> = self
            .cache
            .iter()
            .map(|entry| {
                let guard = entry.value().lock().unwrap();
                (
                    entry.key().clone(),
                    guard.access_count,
                    guard.memory_usage(),
                )
            })
            .collect();

        entries.sort_by_key(|(_, access_count, _)| *access_count);

        let target_memory = self.config.max_cache_memory * 3 / 4; // Reduce to 75%
        let mut current_memory = self.memory_usage();

        for (key, _, memory) in entries {
            if current_memory <= target_memory {
                break;
            }
            self.remove(&key);
            current_memory = current_memory.saturating_sub(memory);
        }
    }

    /// Evict expired entries
    fn evict_expired(&self, ttl: Duration) {
        let expired_keys: Vec<String> = self
            .cache
            .iter()
            .filter_map(|entry| {
                if entry.value().lock().unwrap().is_expired(ttl) {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in expired_keys {
            self.remove(&key);
        }
    }
}

/// Cache statistics for monitoring performance
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Number of cache insertions
    pub cache_inserts: u64,
    /// Number of cache evictions
    pub cache_evictions: u64,
}

impl CacheStats {
    /// Create new cache statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate cache hit ratio
    pub fn hit_ratio(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Get total cache operations
    pub fn total_operations(&self) -> u64 {
        self.cache_hits + self.cache_misses
    }
}

/// Performance statistics for expression evaluation
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    /// Total number of expressions evaluated
    pub expressions_evaluated: u64,
    /// Total evaluation time
    pub total_evaluation_time: Duration,
    /// Number of parse operations
    pub parse_operations: u64,
    /// Total parse time
    pub total_parse_time: Duration,
    /// Number of compilation operations
    pub compilation_operations: u64,
    /// Total compilation time
    pub total_compilation_time: Duration,
    /// Cache statistics
    pub cache_stats: CacheStats,
}

impl PerformanceStats {
    /// Create new performance statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate average evaluation time
    pub fn average_evaluation_time(&self) -> Duration {
        if self.expressions_evaluated == 0 {
            Duration::ZERO
        } else {
            self.total_evaluation_time / self.expressions_evaluated as u32
        }
    }

    /// Calculate average parse time
    pub fn average_parse_time(&self) -> Duration {
        if self.parse_operations == 0 {
            Duration::ZERO
        } else {
            self.total_parse_time / self.parse_operations as u32
        }
    }

    /// Calculate average compilation time
    pub fn average_compilation_time(&self) -> Duration {
        if self.compilation_operations == 0 {
            Duration::ZERO
        } else {
            self.total_compilation_time / self.compilation_operations as u32
        }
    }

    /// Calculate expressions per second
    pub fn expressions_per_second(&self) -> f64 {
        if self.total_evaluation_time.is_zero() {
            0.0
        } else {
            self.expressions_evaluated as f64 / self.total_evaluation_time.as_secs_f64()
        }
    }
}

/// Performance monitor for tracking expression evaluation metrics
#[derive(Debug)]
pub struct PerformanceMonitor {
    stats: Arc<RwLock<PerformanceStats>>,
    config: PerformanceConfig,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            stats: Arc::new(RwLock::new(PerformanceStats::new())),
            config,
        }
    }

    /// Record expression evaluation
    pub fn record_evaluation(&self, duration: Duration) {
        if self.config.is_metrics_enabled() {
            let mut stats = self.stats.write().unwrap();
            stats.expressions_evaluated += 1;
            stats.total_evaluation_time += duration;
        }
    }

    /// Record parse operation
    pub fn record_parse(&self, duration: Duration) {
        if self.config.is_metrics_enabled() {
            let mut stats = self.stats.write().unwrap();
            stats.parse_operations += 1;
            stats.total_parse_time += duration;
        }
    }

    /// Record compilation operation
    pub fn record_compilation(&self, duration: Duration) {
        if self.config.is_metrics_enabled() {
            let mut stats = self.stats.write().unwrap();
            stats.compilation_operations += 1;
            stats.total_compilation_time += duration;
        }
    }

    /// Update cache statistics
    pub fn update_cache_stats(&self, cache_stats: CacheStats) {
        if self.config.is_metrics_enabled() {
            self.stats.write().unwrap().cache_stats = cache_stats;
        }
    }

    /// Get current performance statistics
    pub fn stats(&self) -> PerformanceStats {
        self.stats.read().unwrap().clone()
    }

    /// Reset performance statistics
    pub fn reset_stats(&self) {
        *self.stats.write().unwrap() = PerformanceStats::new();
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new(PerformanceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParsedExpression;
    use pretty_assertions::assert_eq;
    use std::thread;

    fn create_test_expression(source: &str) -> ParsedExpression {
        // Create a simple test expression
        ParsedExpression::simple(source.to_string())
    }

    #[test]
    fn test_performance_config_creation() {
        let fixture = PerformanceConfig::new()
            .with_cache_size(500)
            .with_compilation_enabled(false);

        let actual = fixture.cache_size();
        let expected = 500;
        assert_eq!(actual, expected);

        let actual = fixture.is_compilation_enabled();
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_cache_basic_operations() {
        let fixture = ExpressionCache::new(10);
        let expression = create_test_expression("x + y");

        fixture.insert("test".to_string(), expression.clone());
        let actual = fixture.get("test").unwrap();
        let expected = expression;
        assert_eq!(actual.source(), expected.source());
    }

    #[test]
    fn test_expression_cache_size_limit() {
        let fixture = ExpressionCache::new(2);

        fixture.insert("expr1".to_string(), create_test_expression("1"));
        fixture.insert("expr2".to_string(), create_test_expression("2"));
        fixture.insert("expr3".to_string(), create_test_expression("3"));

        let actual = fixture.size();
        let expected = 2;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_cache_ttl() {
        let config = PerformanceConfig::new().with_cache_ttl(Some(Duration::from_millis(10)));
        let fixture = ExpressionCache::with_config(config);

        fixture.insert("test".to_string(), create_test_expression("x"));

        // Should be available immediately
        assert!(fixture.get("test").is_some());

        // Wait for TTL to expire
        thread::sleep(Duration::from_millis(20));

        // Should be expired now
        let actual = fixture.get("test");
        assert!(actual.is_none());
    }

    #[test]
    fn test_cache_stats() {
        let fixture = ExpressionCache::new(10);
        let expression = create_test_expression("x + y");

        fixture.insert("test".to_string(), expression);
        fixture.get("test"); // hit
        fixture.get("nonexistent"); // miss

        let stats = fixture.stats();
        let actual_hits = stats.cache_hits;
        let actual_misses = stats.cache_misses;
        let expected_hits = 1;
        let expected_misses = 1;

        assert_eq!(actual_hits, expected_hits);
        assert_eq!(actual_misses, expected_misses);
    }

    #[test]
    fn test_cache_hit_ratio() {
        let mut fixture = CacheStats::new();
        fixture.cache_hits = 8;
        fixture.cache_misses = 2;

        let actual = fixture.hit_ratio();
        let expected = 0.8;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_performance_stats() {
        let mut fixture = PerformanceStats::new();
        fixture.expressions_evaluated = 100;
        fixture.total_evaluation_time = Duration::from_secs(10);

        let actual = fixture.average_evaluation_time();
        let expected = Duration::from_millis(100);
        assert_eq!(actual, expected);

        let actual = fixture.expressions_per_second();
        let expected = 10.0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_performance_monitor() {
        let config = PerformanceConfig::new().with_metrics_enabled(true);
        let fixture = PerformanceMonitor::new(config);

        fixture.record_evaluation(Duration::from_millis(100));
        fixture.record_parse(Duration::from_millis(50));

        let stats = fixture.stats();
        let actual_evaluations = stats.expressions_evaluated;
        let actual_parses = stats.parse_operations;
        let expected_evaluations = 1;
        let expected_parses = 1;

        assert_eq!(actual_evaluations, expected_evaluations);
        assert_eq!(actual_parses, expected_parses);
    }

    #[test]
    fn test_cache_memory_usage() {
        let fixture = ExpressionCache::new(10);
        let expression = create_test_expression("x + y");

        let initial_usage = fixture.memory_usage();
        fixture.insert("test".to_string(), expression);
        let after_insert = fixture.memory_usage();

        assert!(after_insert > initial_usage);
    }

    #[test]
    fn test_cache_clear() {
        let fixture = ExpressionCache::new(10);
        fixture.insert("test1".to_string(), create_test_expression("1"));
        fixture.insert("test2".to_string(), create_test_expression("2"));

        assert_eq!(fixture.size(), 2);

        fixture.clear();

        let actual = fixture.size();
        let expected = 0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_performance_monitor_disabled_metrics() {
        let config = PerformanceConfig::new().with_metrics_enabled(false);
        let fixture = PerformanceMonitor::new(config);

        fixture.record_evaluation(Duration::from_millis(100));

        let stats = fixture.stats();
        let actual = stats.expressions_evaluated;
        let expected = 0; // Should not record when disabled
        assert_eq!(actual, expected);
    }
}
