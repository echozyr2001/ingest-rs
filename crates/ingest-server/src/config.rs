use crate::cli::Args;
use crate::error::{ConfigError, ConfigResult};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Complete server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
#[derive(Default)]
pub struct ServerConfig {
    pub server: ServerSettings,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub api: ApiConfig,
    pub connect: ConnectConfig,
    pub execution: ExecutionConfig,
    pub queue: QueueConfig,
    pub pubsub: PubSubConfig,
    pub telemetry: TelemetryConfig,
    pub monitoring: MonitoringConfig,
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ServerSettings {
    pub bind_address: String,
    pub port: u16,
    pub workers: usize,
    pub max_connections: usize,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub startup_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connect_timeout: Duration,
    pub command_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub request_timeout: Duration,
    pub max_request_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ConnectConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub max_connections: usize,
    pub heartbeat_interval: Duration,
    pub connection_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ExecutionConfig {
    pub enabled: bool,
    pub max_concurrent_functions: usize,
    pub max_concurrent_steps: usize,
    pub step_timeout: Duration,
    pub function_timeout: Duration,
    pub retry_max_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct QueueConfig {
    pub enabled: bool,
    pub max_queue_size: usize,
    pub batch_size: usize,
    pub poll_interval: Duration,
    pub visibility_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct PubSubConfig {
    pub enabled: bool,
    pub max_subscribers: usize,
    pub message_buffer_size: usize,
    pub publish_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub tracing_endpoint: Option<String>,
    pub metrics_endpoint: Option<String>,
    pub service_name: String,
    pub sample_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub health_check_interval: Duration,
    pub metrics_port: u16,
    pub prometheus_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct FeatureFlags {
    pub api_enabled: bool,
    pub connect_enabled: bool,
    pub execution_enabled: bool,
    pub queue_enabled: bool,
    pub pubsub_enabled: bool,
    pub history_enabled: bool,
    pub telemetry_enabled: bool,
    pub monitoring_enabled: bool,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            workers: num_cpus::get(),
            max_connections: 10000,
            request_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            startup_timeout: Duration::from_secs(60),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost:5432/inngest".to_string(),
            max_connections: 100,
            min_connections: 5,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 50,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(1),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            cors_origins: vec!["*".to_string()],
            request_timeout: Duration::from_secs(30),
            max_request_size: 1024 * 1024, // 1MB
        }
    }
}

impl Default for ConnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "0.0.0.0".to_string(),
            port: 8081,
            max_connections: 10000,
            heartbeat_interval: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(60),
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_functions: 1000,
            max_concurrent_steps: 10000,
            step_timeout: Duration::from_secs(300),
            function_timeout: Duration::from_secs(3600),
            retry_max_attempts: 3,
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_queue_size: 100000,
            batch_size: 100,
            poll_interval: Duration::from_millis(100),
            visibility_timeout: Duration::from_secs(300),
        }
    }
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_subscribers: 10000,
            message_buffer_size: 1000,
            publish_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tracing_endpoint: None,
            metrics_endpoint: None,
            service_name: "ingest-server".to_string(),
            sample_rate: 0.1,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            health_check_interval: Duration::from_secs(30),
            metrics_port: 9090,
            prometheus_enabled: true,
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            api_enabled: true,
            connect_enabled: true,
            execution_enabled: true,
            queue_enabled: true,
            pubsub_enabled: true,
            history_enabled: true,
            telemetry_enabled: true,
            monitoring_enabled: true,
        }
    }
}

impl ServerConfig {
    /// Load configuration from multiple sources
    pub fn load(args: &Args) -> ConfigResult<Self> {
        let mut config = Self::default();

        // Load from file if specified
        if let Some(config_file) = args.config_file() {
            let file_config = Self::from_file(config_file)?;
            config = config.merge(file_config)?;
        }

        // Override with environment variables
        config.merge_with_env()?;

        // Override with CLI arguments
        config.merge_with_args(args)?;

        Ok(config)
    }

    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> ConfigResult<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(ConfigError::Io)?;

        let config: Self = toml::from_str(&content).map_err(ConfigError::Toml)?;

        Ok(config)
    }

    /// Merge with environment variables
    pub fn merge_with_env(&mut self) -> ConfigResult<()> {
        // Server settings
        if let Ok(bind_address) = std::env::var("INGEST_BIND_ADDRESS") {
            self.server.bind_address = bind_address;
        }
        if let Ok(port) = std::env::var("INGEST_PORT") {
            self.server.port = port.parse().map_err(|_| ConfigError::InvalidValue {
                field: "INGEST_PORT".to_string(),
                value: port,
            })?;
        }

        // Database settings
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            self.database.url = database_url;
        }

        // Redis settings
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            self.redis.url = redis_url;
        }

        Ok(())
    }

    /// Merge with CLI arguments
    pub fn merge_with_args(&mut self, args: &Args) -> ConfigResult<()> {
        self.server.bind_address = args.bind_address.clone();
        self.server.port = args.port;

        if let Some(database_url) = &args.database_url {
            self.database.url = database_url.clone();
        }

        if let Some(redis_url) = &args.redis_url {
            self.redis.url = redis_url.clone();
        }

        Ok(())
    }

    /// Merge with another configuration
    pub fn merge(mut self, other: Self) -> ConfigResult<Self> {
        // For simplicity, we'll replace entire sections
        // In a production system, you might want more granular merging
        self.server = other.server;
        self.database = other.database;
        self.redis = other.redis;
        self.api = other.api;
        self.connect = other.connect;
        self.execution = other.execution;
        self.queue = other.queue;
        self.pubsub = other.pubsub;
        self.telemetry = other.telemetry;
        self.monitoring = other.monitoring;
        self.features = other.features;

        Ok(self)
    }

    /// Validate the configuration
    pub fn validate(&self) -> ConfigResult<()> {
        // Validate server settings
        if self.server.port == 0 {
            return Err(ConfigError::InvalidValue {
                field: "server.port".to_string(),
                value: "0".to_string(),
            });
        }

        if self.server.workers == 0 {
            return Err(ConfigError::InvalidValue {
                field: "server.workers".to_string(),
                value: "0".to_string(),
            });
        }

        // Validate database URL
        if self.database.url.is_empty() {
            return Err(ConfigError::MissingRequired("database.url".to_string()));
        }

        // Validate Redis URL
        if self.redis.url.is_empty() {
            return Err(ConfigError::MissingRequired("redis.url".to_string()));
        }

        // Validate feature consistency
        if self.features.api_enabled && !self.api.enabled {
            return Err(ConfigError::Validation(
                "API feature is enabled but API service is disabled".to_string(),
            ));
        }

        Ok(())
    }

    /// Generate a default configuration file
    pub fn generate_default() -> ConfigResult<String> {
        let config = Self::default();
        toml::to_string_pretty(&config).map_err(|e| ConfigError::InvalidFile(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config() {
        let fixture = ServerConfig::default();
        let actual = fixture.server.port;
        let expected = 8080;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_config_validation_success() {
        let fixture = ServerConfig::default();
        let actual = fixture.validate().is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut fixture = ServerConfig::default();
        fixture.server.port = 0;
        let actual = fixture.validate().is_err();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_merge_with_args() {
        let mut fixture = ServerConfig::default();
        let args = Args {
            config: None,
            bind_address: "127.0.0.1".to_string(),
            port: 9000,
            log_level: "debug".to_string(),
            dev_mode: false,
            database_url: Some("postgresql://test:5432/test".to_string()),
            redis_url: None,
            command: None,
        };

        fixture.merge_with_args(&args).unwrap();

        let actual = fixture.server.bind_address;
        let expected = "127.0.0.1";
        assert_eq!(actual, expected);

        let actual = fixture.server.port;
        let expected = 9000;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_generate_default_config() {
        let fixture = ServerConfig::generate_default();
        let actual = fixture.is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }
}
