//! Configuration for Redis state management.

use crate::error::{StateError, StateResult};
use fred::types::config::{Config, ServerConfig};
use std::time::Duration;

/// Configuration for Redis state management
#[derive(Debug, Clone)]
pub struct StateConfig {
    /// Redis connection configuration
    pub redis_config: Config,

    /// Key prefix for all Redis keys (default: "inngest")
    pub key_prefix: String,

    /// Whether to use sharded keys (default: false)
    pub sharded_keys: bool,

    /// Connection pool size (default: 10)
    pub pool_size: u32,

    /// Connection timeout (default: 5 seconds)
    pub connect_timeout: Duration,

    /// Command timeout (default: 30 seconds)  
    pub command_timeout: Duration,

    /// Maximum number of retries for failed operations (default: 3)
    pub max_retries: u32,

    /// Base delay for exponential backoff (default: 100ms)
    pub retry_delay: Duration,

    /// Whether to enable tracing for Redis operations (default: true)
    pub enable_tracing: bool,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            redis_config: Config {
                server: ServerConfig::new_centralized("127.0.0.1", 6379),
                ..Default::default()
            },
            key_prefix: "inngest".to_string(),
            sharded_keys: false,
            pool_size: 10,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
            enable_tracing: true,
        }
    }
}

impl StateConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set Redis URL
    pub fn with_redis_url(mut self, url: &str) -> StateResult<Self> {
        // First validate that the URL has a valid Redis protocol
        if !url.starts_with("redis://") && !url.starts_with("rediss://") {
            return Err(StateError::config(format!(
                "Invalid Redis URL: must start with 'redis://' or 'rediss://'. Got: {url}"
            )));
        }

        // Then try to parse with fred
        self.redis_config = Config::from_url(url)
            .map_err(|e| StateError::config(format!("Invalid Redis URL: {e}")))?;
        Ok(self)
    }

    /// Set Redis host and port
    pub fn with_redis_host(mut self, host: &str, port: u16) -> Self {
        self.redis_config.server = ServerConfig::new_centralized(host, port);
        self
    }

    /// Set key prefix
    pub fn with_key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    /// Enable sharded keys
    pub fn with_sharded_keys(mut self, sharded: bool) -> Self {
        self.sharded_keys = sharded;
        self
    }

    /// Set connection pool size
    pub fn with_pool_size(mut self, size: u32) -> Self {
        self.pool_size = size;
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set command timeout
    pub fn with_command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set retry delay
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Enable or disable tracing
    pub fn with_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> StateResult<()> {
        if self.key_prefix.is_empty() {
            return Err(StateError::config("Key prefix cannot be empty"));
        }

        if self.pool_size == 0 {
            return Err(StateError::config("Pool size must be greater than 0"));
        }

        if self.max_retries > 10 {
            return Err(StateError::config("Max retries should not exceed 10"));
        }

        Ok(())
    }
}

/// Redis connection information for debugging
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub server: String,
    pub database: Option<u8>,
    pub pool_size: u32,
    pub connected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config() {
        let config = StateConfig::default();
        assert_eq!(config.key_prefix, "inngest");
        assert_eq!(config.pool_size, 10);
        assert!(!config.sharded_keys);
        assert!(config.enable_tracing);
    }

    #[test]
    fn test_config_builder() {
        let config = StateConfig::new()
            .with_key_prefix("test")
            .with_pool_size(5)
            .with_sharded_keys(true)
            .with_tracing(false);

        assert_eq!(config.key_prefix, "test");
        assert_eq!(config.pool_size, 5);
        assert!(config.sharded_keys);
        assert!(!config.enable_tracing);
    }

    #[test]
    fn test_redis_url_parsing() {
        let _config = StateConfig::new()
            .with_redis_url("redis://localhost:6380/1")
            .unwrap();

        // The URL should be parsed correctly
        // Note: We can't easily test the internal RedisConfig structure,
        // but we know it parsed successfully if no error was returned
    }

    #[test]
    fn test_invalid_redis_url() {
        let result = StateConfig::new().with_redis_url("invalid://url");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid Redis URL")
        );
    }

    #[test]
    fn test_config_validation() {
        // Valid config
        let config = StateConfig::default();
        assert!(config.validate().is_ok());

        // Empty prefix
        let config = StateConfig::default().with_key_prefix("");
        assert!(config.validate().is_err());

        // Zero pool size
        let config = StateConfig::default().with_pool_size(0);
        assert!(config.validate().is_err());

        // Too many retries
        let config = StateConfig::default().with_max_retries(20);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_redis_host_config() {
        let config = StateConfig::new().with_redis_host("redis.example.com", 6380);

        // We can't easily inspect the internal ServerConfig,
        // but we can verify the config was created without error
        assert_eq!(config.key_prefix, "inngest"); // Other fields should remain default
    }
}
