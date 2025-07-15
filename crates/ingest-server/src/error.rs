use thiserror::Error;

/// Errors that can occur in the ingest server
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Service error: {service} - {error}")]
    Service { service: String, error: String },

    #[error("Startup timeout exceeded")]
    StartupTimeout,

    #[error("Shutdown timeout exceeded")]
    ShutdownTimeout,

    #[error("Health check failed: {0}")]
    HealthCheck(String),

    #[error("Signal handling error: {0}")]
    Signal(String),

    #[error("Storage error: {0}")]
    Storage(#[from] Box<ingest_storage::error::StorageError>),

    #[error("API error: {0}")]
    Api(#[from] Box<ingest_api::error::ApiError>),

    #[error("Connect error: {0}")]
    Connect(#[from] Box<ingest_connect::error::ConnectError>),

    #[error("Execution error: {0}")]
    Execution(#[from] Box<ingest_execution::error::ExecutionError>),

    #[error("Telemetry error: {0}")]
    Telemetry(#[from] Box<ingest_telemetry::error::TelemetryError>),

    #[error("Monitoring error: {0}")]
    Monitoring(#[from] Box<ingest_monitoring::error::MonitoringError>),
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration file: {0}")]
    InvalidFile(String),

    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    #[error("Invalid configuration value: {field} = {value}")]
    InvalidValue { field: String, value: String },

    #[error("Environment variable error: {0}")]
    Environment(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Config parsing error: {0}")]
    ConfigParsing(#[from] config::ConfigError),
}

pub type Result<T> = std::result::Result<T, ServerError>;
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_server_error_display() {
        let fixture = ServerError::StartupTimeout;
        let actual = fixture.to_string();
        let expected = "Startup timeout exceeded";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_config_error_display() {
        let fixture = ConfigError::MissingRequired("database.url".to_string());
        let actual = fixture.to_string();
        let expected = "Missing required configuration: database.url";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_service_error_display() {
        let fixture = ServerError::Service {
            service: "api".to_string(),
            error: "failed to bind to port".to_string(),
        };
        let actual = fixture.to_string();
        let expected = "Service error: api - failed to bind to port";
        assert_eq!(actual, expected);
    }
}
