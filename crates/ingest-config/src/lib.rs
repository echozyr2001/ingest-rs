//! # Inngest Configuration
//!
//! This crate provides configuration management for the Inngest platform,
//! supporting multiple configuration sources and formats.
//!
//! ## Features
//!
//! - Environment variable configuration
//! - TOML file configuration
//! - Configuration validation
//! - Feature flags and toggles
//! - Service discovery configuration

pub mod database;
pub mod error;
pub mod features;
pub mod loader;
pub mod server;

use derive_setters::Setters;
use serde::{Deserialize, Serialize};

pub use database::DatabaseConfig;
pub use error::{ConfigError, Result};
pub use features::FeatureFlags;
pub use loader::ConfigLoader;
pub use server::ServerConfig;

/// Main configuration structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
#[derive(Default)]
pub struct Config {
    /// Database configuration
    pub database: DatabaseConfig,
    /// Redis configuration
    pub redis: RedisConfig,
    /// Server configuration
    pub server: ServerConfig,
    /// Feature flags
    pub features: FeatureFlags,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Redis configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Maximum number of connections in pool
    pub max_connections: Option<u32>,
    /// Connection timeout
    pub connect_timeout: Option<std::time::Duration>,
    /// Command timeout
    pub command_timeout: Option<std::time::Duration>,
    /// Enable TLS
    pub tls: Option<bool>,
}

/// Logging configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Log format (json, pretty)
    pub format: String,
    /// Enable structured logging
    pub structured: bool,
    /// OpenTelemetry endpoint
    pub otlp_endpoint: Option<String>,
    /// Service name for tracing
    pub service_name: String,
}

/// Security configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct SecurityConfig {
    /// JWT secret key
    pub jwt_secret: Option<String>,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Enable CORS
    pub cors_enabled: bool,
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    /// Enable rate limiting
    pub rate_limiting: bool,
    /// Rate limit requests per minute
    pub rate_limit_rpm: Option<u32>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: Some(10),
            connect_timeout: Some(std::time::Duration::from_secs(5)),
            command_timeout: Some(std::time::Duration::from_secs(30)),
            tls: Some(false),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            structured: true,
            otlp_endpoint: None,
            service_name: "inngest".to_string(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: None,
            api_key: None,
            cors_enabled: true,
            cors_origins: vec!["*".to_string()],
            rate_limiting: false,
            rate_limit_rpm: Some(1000),
        }
    }
}

impl Config {
    /// Load configuration from default sources
    pub fn load() -> Result<Self> {
        ConfigLoader::new().load()
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &str) -> Result<Self> {
        ConfigLoader::new().with_file(path).load()
    }

    /// Load configuration with custom environment prefix
    pub fn load_with_prefix(prefix: &str) -> Result<Self> {
        ConfigLoader::new().with_env_prefix(prefix).load()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.database.validate()?;
        self.server.validate()?;
        self.validate_redis()?;
        self.validate_logging()?;
        self.validate_security()?;
        Ok(())
    }

    fn validate_redis(&self) -> Result<()> {
        if self.redis.url.is_empty() {
            return Err(ConfigError::validation("Redis URL cannot be empty"));
        }

        url::Url::parse(&self.redis.url)
            .map_err(|e| ConfigError::validation(format!("Invalid Redis URL: {e}")))?;

        if let Some(max_conn) = self.redis.max_connections {
            if max_conn == 0 {
                return Err(ConfigError::validation("Redis max_connections must be > 0"));
            }
        }

        Ok(())
    }

    fn validate_logging(&self) -> Result<()> {
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(ConfigError::validation(format!(
                "Invalid log level: {}. Must be one of: {}",
                self.logging.level,
                valid_levels.join(", ")
            )));
        }

        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.logging.format.as_str()) {
            return Err(ConfigError::validation(format!(
                "Invalid log format: {}. Must be one of: {}",
                self.logging.format,
                valid_formats.join(", ")
            )));
        }

        if self.logging.service_name.is_empty() {
            return Err(ConfigError::validation("Service name cannot be empty"));
        }

        Ok(())
    }

    fn validate_security(&self) -> Result<()> {
        if let Some(rpm) = self.security.rate_limit_rpm {
            if rpm == 0 {
                return Err(ConfigError::validation("Rate limit RPM must be > 0"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_config_default() {
        let actual = Config::default();
        assert_eq!(actual.database.host, "localhost");
        assert_eq!(actual.redis.url, "redis://localhost:6379");
        assert_eq!(actual.server.host, "0.0.0.0");
        assert_eq!(actual.logging.level, "info");
    }

    #[test]
    fn test_redis_config_default() {
        let actual = RedisConfig::default();
        let expected = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: Some(10),
            connect_timeout: Some(std::time::Duration::from_secs(5)),
            command_timeout: Some(std::time::Duration::from_secs(30)),
            tls: Some(false),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_logging_config_default() {
        let actual = LoggingConfig::default();
        assert_eq!(actual.level, "info");
        assert_eq!(actual.format, "json");
        assert!(actual.structured);
        assert_eq!(actual.service_name, "inngest");
    }

    #[test]
    fn test_security_config_default() {
        let actual = SecurityConfig::default();
        assert!(actual.cors_enabled);
        assert_eq!(actual.cors_origins, vec!["*".to_string()]);
        assert!(!actual.rate_limiting);
        assert_eq!(actual.rate_limit_rpm, Some(1000));
    }

    #[test]
    fn test_config_setters() {
        let actual = Config::default()
            .logging(LoggingConfig::default().level("debug"))
            .security(SecurityConfig::default().cors_enabled(false));

        assert_eq!(actual.logging.level, "debug");
        assert!(!actual.security.cors_enabled);
    }

    #[test]
    fn test_config_validation_success() {
        let fixture = Config::default();
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_config_validation_invalid_redis_url() {
        let fixture = Config::default().redis(RedisConfig::default().url("invalid-url"));

        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let fixture = Config::default().logging(LoggingConfig::default().level("invalid"));

        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_config_serialization() {
        let fixture = Config::default();
        let actual = toml::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_config_deserialization() {
        let fixture = Config::default();
        let serialized = toml::to_string(&fixture).unwrap();
        let actual: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(actual.database.host, fixture.database.host);
        assert_eq!(actual.redis.url, fixture.redis.url);
    }
}
