use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub event_stream: EventStreamConfig,
    pub api: ApiConfig,
    pub dev_server: DevServerConfig,
    pub production: ProductionConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            event_stream: EventStreamConfig::default(),
            api: ApiConfig::default(),
            dev_server: DevServerConfig::default(),
            production: ProductionConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();

        // Database configuration
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.database.url = url;
        }
        if let Ok(max_conn) = std::env::var("DATABASE_MAX_CONNECTIONS") {
            config.database.max_connections = max_conn.parse()?;
        }
        if let Ok(min_conn) = std::env::var("DATABASE_MIN_CONNECTIONS") {
            config.database.min_connections = min_conn.parse()?;
        }

        // Redis configuration
        if let Ok(url) = std::env::var("REDIS_URL") {
            config.redis.url = url;
        }
        if let Ok(max_conn) = std::env::var("REDIS_MAX_CONNECTIONS") {
            config.redis.max_connections = max_conn.parse()?;
        }

        // Production configuration
        if let Ok(env) = std::env::var("ENVIRONMENT") {
            config.production.environment = env;
        }
        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.production.log_level = log_level;
        }
        if let Ok(metrics) = std::env::var("METRICS_ENABLED") {
            config.production.metrics_enabled = metrics.parse().unwrap_or(false);
        }
        if let Ok(tracing) = std::env::var("TRACING_ENABLED") {
            config.production.tracing_enabled = tracing.parse().unwrap_or(false);
        }
        if let Ok(jaeger) = std::env::var("JAEGER_ENDPOINT") {
            config.production.jaeger_endpoint = Some(jaeger);
        }

        Ok(config)
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost:5432/inngest".to_string(),
            max_connections: 20,
            min_connections: 5,
            acquire_timeout_secs: 30,
            idle_timeout_secs: Some(600),  // 10 minutes
            max_lifetime_secs: Some(1800), // 30 minutes
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 20,
            pool_timeout_secs: 30,
            connection_timeout_secs: 10,
            command_timeout_secs: 30,
        }
    }
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            environment: "development".to_string(),
            log_level: "info".to_string(),
            metrics_enabled: false,
            tracing_enabled: false,
            jaeger_endpoint: None,
            health_check_interval_secs: 30,
            graceful_shutdown_timeout_secs: 30,
        }
    }
}

impl Default for EventStreamConfig {
    fn default() -> Self {
        Self {
            service_type: "memory".to_string(),
            topic_name: "inngest-events".to_string(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

impl Default for DevServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1".to_string(),
            port: 8288,
            poll_interval: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
    pub idle_timeout_secs: Option<u64>,
    pub max_lifetime_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub pool_timeout_secs: u64,
    pub connection_timeout_secs: u64,
    pub command_timeout_secs: u64,
}

/// Production-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    pub environment: String,
    pub log_level: String,
    pub metrics_enabled: bool,
    pub tracing_enabled: bool,
    pub jaeger_endpoint: Option<String>,
    pub health_check_interval_secs: u64,
    pub graceful_shutdown_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamConfig {
    pub service_type: String,
    pub topic_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub addr: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServerConfig {
    pub addr: String,
    pub port: u16,
    pub poll_interval: u64,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("INNGEST"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
