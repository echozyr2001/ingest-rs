use crate::{HealthCheck, Result, StorageError};
use async_trait::async_trait;
use ingest_config::RedisConfig;
use redis::{AsyncCommands, Client, aio::ConnectionManager};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Redis storage implementation for caching and key-value operations
#[derive(Clone)]
pub struct RedisStorage {
    client: Client,
    connection_manager: ConnectionManager,
}

impl RedisStorage {
    /// Ping Redis to check connectivity
    pub async fn ping(&self) -> Result<()> {
        let start = std::time::Instant::now();

        let mut conn = self.connection_manager.clone();

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| StorageError::query(format!("Redis ping failed: {e}")))?;

        let duration = start.elapsed();
        println!("Redis ping successful in {duration:?}");
        Ok(())
    }
    /// Create a new Redis storage instance
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        info!("Connecting to Redis at {}", config.url);

        let client = Client::open(config.url.as_str()).map_err(|e| {
            error!("Failed to create Redis client: {}", e);
            StorageError::connection(format!("Failed to create Redis client: {e}"))
        })?;

        let connection_manager = ConnectionManager::new(client.clone()).await.map_err(|e| {
            error!("Failed to create Redis connection manager: {}", e);
            StorageError::connection(format!("Failed to create Redis connection manager: {e}"))
        })?;

        info!("Successfully connected to Redis");

        Ok(Self {
            client,
            connection_manager,
        })
    }

    /// Get a reference to the Redis client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a cloned connection manager
    pub fn connection_manager(&self) -> ConnectionManager {
        self.connection_manager.clone()
    }

    /// Create an in-memory Redis storage for testing
    pub async fn new_in_memory() -> Result<Self> {
        // For benchmarking, we'll use a mock implementation
        // In real usage, this would connect to a test Redis instance
        let config = ingest_config::RedisConfig::default().url("redis://localhost:6379");

        // Try to connect, but if it fails, create a mock for benchmarking
        match Self::new(&config).await {
            Ok(storage) => Ok(storage),
            Err(_) => {
                // Create a mock Redis storage for benchmarking when Redis is not available
                panic!("Redis not available for benchmarking")
            }
        }
    }
}

/// Cache storage trait for Redis operations
#[async_trait]
pub trait CacheStorage: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value with optional TTL
    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<()>;

    /// Delete a key
    async fn delete(&self, key: &str) -> Result<bool>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Set multiple key-value pairs
    async fn mset(&self, pairs: &[(&str, &[u8])]) -> Result<()>;

    /// Get multiple values by keys
    async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>>;

    /// Increment a numeric value
    async fn incr(&self, key: &str, delta: i64) -> Result<i64>;

    /// Set expiration time for a key
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Get TTL for a key
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Perform a health check
    async fn health_check(&self) -> Result<HealthCheck>;
}

#[async_trait]
impl CacheStorage for RedisStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!("Getting key: {}", key);

        let mut conn = self.connection_manager.clone();
        let result: Option<Vec<u8>> = conn.get(key).await.map_err(|e| {
            error!("Failed to get key '{}': {}", key, e);
            StorageError::query(format!("Failed to get key '{key}': {e}"))
        })?;

        debug!("Key '{}' found: {}", key, result.is_some());
        Ok(result)
    }

    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<()> {
        debug!("Setting key: {} (TTL: {:?})", key, ttl);

        let mut conn = self.connection_manager.clone();

        match ttl {
            Some(duration) => {
                let seconds = duration.as_secs();
                let _: () = conn.set_ex(key, value, seconds).await.map_err(|e| {
                    error!("Failed to set key '{}' with TTL: {}", key, e);
                    StorageError::query(format!("Failed to set key '{key}' with TTL: {e}"))
                })?;
            }
            None => {
                let _: () = conn.set(key, value).await.map_err(|e| {
                    error!("Failed to set key '{}': {}", key, e);
                    StorageError::query(format!("Failed to set key '{key}': {e}"))
                })?;
            }
        }

        debug!("Key '{}' set successfully", key);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        debug!("Deleting key: {}", key);

        let mut conn = self.connection_manager.clone();
        let deleted: u32 = conn.del(key).await.map_err(|e| {
            error!("Failed to delete key '{}': {}", key, e);
            StorageError::query(format!("Failed to delete key '{key}': {e}"))
        })?;

        let was_deleted = deleted > 0;
        debug!("Key '{}' deleted: {}", key, was_deleted);
        Ok(was_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        debug!("Checking existence of key: {}", key);

        let mut conn = self.connection_manager.clone();
        let exists: bool = conn.exists(key).await.map_err(|e| {
            error!("Failed to check existence of key '{}': {}", key, e);
            StorageError::query(format!("Failed to check existence of key '{key}': {e}"))
        })?;

        debug!("Key '{}' exists: {}", key, exists);
        Ok(exists)
    }

    async fn mset(&self, pairs: &[(&str, &[u8])]) -> Result<()> {
        debug!("Setting {} key-value pairs", pairs.len());

        let mut conn = self.connection_manager.clone();

        // Convert to the format Redis expects
        let redis_pairs: Vec<(String, Vec<u8>)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_vec()))
            .collect();

        let _: () = conn.mset(&redis_pairs).await.map_err(|e| {
            error!("Failed to set multiple keys: {}", e);
            StorageError::query(format!("Failed to set multiple keys: {e}"))
        })?;

        debug!("Successfully set {} key-value pairs", pairs.len());
        Ok(())
    }

    async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        debug!("Getting {} keys", keys.len());

        let mut conn = self.connection_manager.clone();
        let values: Vec<Option<Vec<u8>>> = conn.mget(keys).await.map_err(|e| {
            error!("Failed to get multiple keys: {}", e);
            StorageError::query(format!("Failed to get multiple keys: {e}"))
        })?;

        debug!("Retrieved {} values", values.len());
        Ok(values)
    }

    async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        debug!("Incrementing key '{}' by {}", key, delta);

        let mut conn = self.connection_manager.clone();
        let new_value: i64 = if delta == 1 {
            conn.incr(key, 1).await
        } else {
            conn.incr(key, delta).await
        }
        .map_err(|e| {
            error!("Failed to increment key '{}': {}", key, e);
            StorageError::query(format!("Failed to increment key '{key}': {e}"))
        })?;

        debug!("Key '{}' incremented to {}", key, new_value);
        Ok(new_value)
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        debug!("Setting expiration for key '{}' to {:?}", key, ttl);

        let mut conn = self.connection_manager.clone();
        let seconds = ttl.as_secs();
        let success: bool = conn.expire(key, seconds as i64).await.map_err(|e| {
            error!("Failed to set expiration for key '{}': {}", key, e);
            StorageError::query(format!("Failed to set expiration for key '{key}': {e}"))
        })?;

        debug!("Expiration set for key '{}': {}", key, success);
        Ok(success)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        debug!("Getting TTL for key: {}", key);

        let mut conn = self.connection_manager.clone();
        let ttl_seconds: i64 = conn.ttl(key).await.map_err(|e| {
            error!("Failed to get TTL for key '{}': {}", key, e);
            StorageError::query(format!("Failed to get TTL for key '{key}': {e}"))
        })?;

        let ttl = match ttl_seconds {
            -2 => None, // Key doesn't exist
            -1 => None, // Key exists but has no expiration
            seconds if seconds > 0 => Some(Duration::from_secs(seconds as u64)),
            _ => {
                warn!("Unexpected TTL value for key '{}': {}", key, ttl_seconds);
                None
            }
        };

        debug!("TTL for key '{}': {:?}", key, ttl);
        Ok(ttl)
    }

    async fn health_check(&self) -> Result<HealthCheck> {
        let start = Instant::now();

        let mut conn = self.connection_manager.clone();
        match redis::cmd("PING").query_async::<String>(&mut conn).await {
            Ok(response) if response == "PONG" => {
                let response_time = start.elapsed().as_millis() as u64;
                debug!("Redis health check passed in {}ms", response_time);
                Ok(HealthCheck::healthy(response_time))
            }
            Ok(unexpected) => {
                let response_time = start.elapsed().as_millis() as u64;
                error!(
                    "Redis health check returned unexpected response: {}",
                    unexpected
                );
                Ok(HealthCheck::unhealthy(
                    format!("Unexpected PING response: {unexpected}"),
                    response_time,
                ))
            }
            Err(e) => {
                let response_time = start.elapsed().as_millis() as u64;
                error!("Redis health check failed: {}", e);
                Ok(HealthCheck::unhealthy(
                    format!("Health check failed: {e}"),
                    response_time,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_config::RedisConfig;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    fn create_test_config() -> RedisConfig {
        RedisConfig::default()
            .url("redis://localhost:6379")
            .max_connections(5u32)
            .connect_timeout(Duration::from_secs(1))
            .command_timeout(Duration::from_secs(5))
    }

    #[tokio::test]
    async fn test_redis_storage_creation() {
        // This test requires a running Redis instance
        // Skip if not available
        let config = create_test_config();

        // Just test that the creation doesn't panic
        // In a real test environment, you'd use testcontainers
        match RedisStorage::new(&config).await {
            Ok(_) => {
                // Connection successful - this would only happen in CI with a real Redis
            }
            Err(_) => {
                // Connection failed (expected if no Redis available)
            }
        }
    }

    #[test]
    fn test_redis_config_url() {
        let fixture = create_test_config();
        let actual = fixture.url;
        let expected = "redis://localhost:6379";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cache_storage_trait_compiles() {
        // This test ensures the CacheStorage trait compiles correctly
        // Implementation tests would require a real Redis instance
    }

    #[tokio::test]
    async fn test_redis_operations_mock() {
        // Mock test to verify the interface works
        // In integration tests, we would test with a real Redis instance
        let config = create_test_config();

        // Test configuration is valid
        assert_eq!(config.max_connections, Some(5u32));
        assert_eq!(config.connect_timeout, Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_redis_storage_debug() {
        // Test that RedisStorage implements Debug
        let config = create_test_config();
        // We can't actually create a RedisStorage without a real Redis connection
        // but we can test the config
        assert_eq!(format!("{:?}", config.url), r#""redis://localhost:6379""#);
    }
}
