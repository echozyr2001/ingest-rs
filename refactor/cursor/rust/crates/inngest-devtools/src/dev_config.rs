//! Development configuration management
//!
//! This module handles configuration for development vs production environments.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Overall development configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevConfig {
    /// Environment type (dev, prod, test)
    pub environment: Environment,
    /// Whether to use in-memory storage
    pub in_memory: bool,
    /// Data directory
    pub data_dir: Option<PathBuf>,
    /// Redis configuration
    pub redis: RedisConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Server configuration
    pub server: ServerConfig,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Development,
            in_memory: true,
            data_dir: None,
            redis: RedisConfig::default(),
            database: DatabaseConfig::default(),
            server: ServerConfig::default(),
        }
    }
}

/// Environment types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "production")]
    Production,
    #[serde(rename = "test")]
    Test,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// External Redis URI
    pub uri: Option<String>,
    /// Use embedded Redis if no URI is provided
    pub use_embedded: bool,
    /// Tick interval for time simulation (ms)
    pub tick_interval_ms: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            uri: None,
            use_embedded: true,
            tick_interval_ms: 150,
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// External database URI (PostgreSQL/MySQL)
    pub uri: Option<String>,
    /// SQLite file path
    pub sqlite_path: Option<PathBuf>,
    /// Use in-memory SQLite
    pub use_memory: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            uri: None,
            sqlite_path: None,
            use_memory: true,
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Enable CORS for development
    pub cors: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8288,
            cors: true,
        }
    }
}

impl DevConfig {
    /// Load configuration from environment variables and files
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Load environment
        if let Ok(env) = std::env::var("INNGEST_ENV") {
            config.environment = match env.as_str() {
                "production" | "prod" => Environment::Production,
                "test" => Environment::Test,
                _ => Environment::Development,
            };
        }

        // Load in-memory setting
        if let Ok(in_memory) = std::env::var("INNGEST_IN_MEMORY") {
            config.in_memory = in_memory.parse().unwrap_or(true);
        }

        // Load data directory
        if let Ok(data_dir) = std::env::var("INNGEST_DATA_DIR") {
            config.data_dir = Some(PathBuf::from(data_dir));
        }

        // Load Redis URI
        if let Ok(redis_uri) = std::env::var("REDIS_URL") {
            config.redis.uri = Some(redis_uri);
            config.redis.use_embedded = false;
        }

        // Load database URI
        if let Ok(db_uri) = std::env::var("DATABASE_URL") {
            config.database.uri = Some(db_uri);
            config.database.use_memory = false;
        }

        // Load SQLite path
        if let Ok(sqlite_path) = std::env::var("SQLITE_PATH") {
            config.database.sqlite_path = Some(PathBuf::from(sqlite_path));
            config.database.use_memory = false;
        }

        // Load server configuration
        if let Ok(host) = std::env::var("INNGEST_HOST") {
            config.server.host = host;
        }

        if let Ok(port) = std::env::var("INNGEST_PORT") {
            config.server.port = port.parse().unwrap_or(8288);
        }

        config
    }

    /// Get effective data directory
    pub fn effective_data_dir(&self) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./inngest-data"))
    }

    /// Get effective SQLite path
    pub fn effective_sqlite_path(&self) -> PathBuf {
        if let Some(path) = &self.database.sqlite_path {
            path.clone()
        } else {
            self.effective_data_dir().join("inngest.db")
        }
    }

    /// Check if using external services
    pub fn uses_external_redis(&self) -> bool {
        self.redis.uri.is_some()
    }

    pub fn uses_external_database(&self) -> bool {
        self.database.uri.is_some()
    }

    /// Get Redis connection string
    pub fn redis_connection_string(&self) -> String {
        self.redis
            .uri
            .clone()
            .unwrap_or_else(|| "embedded://localhost:0".to_string())
    }

    /// Get database connection string
    pub fn database_connection_string(&self) -> String {
        if let Some(uri) = &self.database.uri {
            uri.clone()
        } else if self.database.use_memory {
            "sqlite://:memory:".to_string()
        } else {
            format!("sqlite://{}", self.effective_sqlite_path().display())
        }
    }
}

/// Development environment
pub use crate::DevEnvironment;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config() {
        let config = DevConfig::default();
        assert_eq!(config.environment, Environment::Development);
        assert!(config.in_memory);
        assert!(config.redis.use_embedded);
        assert!(config.database.use_memory);
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("INNGEST_ENV", "production");
        std::env::set_var("INNGEST_IN_MEMORY", "false");
        std::env::set_var("REDIS_URL", "redis://localhost:6379");

        let config = DevConfig::from_env();
        assert_eq!(config.environment, Environment::Production);
        assert!(!config.in_memory);
        assert_eq!(config.redis.uri, Some("redis://localhost:6379".to_string()));
        assert!(!config.redis.use_embedded);

        // Cleanup
        std::env::remove_var("INNGEST_ENV");
        std::env::remove_var("INNGEST_IN_MEMORY");
        std::env::remove_var("REDIS_URL");
    }

    #[test]
    fn test_connection_strings() {
        let config = DevConfig::default();
        assert_eq!(config.redis_connection_string(), "embedded://localhost:0");
        assert_eq!(config.database_connection_string(), "sqlite://:memory:");
    }
}
