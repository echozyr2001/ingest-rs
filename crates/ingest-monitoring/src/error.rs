use thiserror::Error;

/// Monitoring error types
#[derive(Error, Debug)]
pub enum MonitoringError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Initialization error
    #[error("Initialization error: {0}")]
    Initialization(String),

    /// Health check error
    #[error("Health check error: {0}")]
    HealthCheck(String),

    /// Resource monitoring error
    #[error("Resource monitoring error: {0}")]
    Resource(String),

    /// HTTP server error
    #[error("HTTP server error: {0}")]
    Http(String),

    /// Metrics error
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Monitoring error: {0}")]
    Other(String),
}

impl MonitoringError {
    /// Create a configuration error
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    /// Create an initialization error
    pub fn initialization(msg: impl Into<String>) -> Self {
        Self::Initialization(msg.into())
    }

    /// Create a health check error
    pub fn health_check(msg: impl Into<String>) -> Self {
        Self::HealthCheck(msg.into())
    }

    /// Create a resource monitoring error
    pub fn resource(msg: impl Into<String>) -> Self {
        Self::Resource(msg.into())
    }

    /// Create an HTTP server error
    pub fn http(msg: impl Into<String>) -> Self {
        Self::Http(msg.into())
    }

    /// Create a metrics error
    pub fn metrics(msg: impl Into<String>) -> Self {
        Self::Metrics(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Result type for monitoring operations
pub type Result<T> = std::result::Result<T, MonitoringError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_monitoring_error_configuration() {
        let error = MonitoringError::configuration("test error");
        assert_eq!(error.to_string(), "Configuration error: test error");
    }

    #[test]
    fn test_monitoring_error_initialization() {
        let error = MonitoringError::initialization("init failed");
        assert_eq!(error.to_string(), "Initialization error: init failed");
    }

    #[test]
    fn test_monitoring_error_health_check() {
        let error = MonitoringError::health_check("health check failed");
        assert_eq!(error.to_string(), "Health check error: health check failed");
    }

    #[test]
    fn test_result_type() {
        let failure: Result<i32> = Err(MonitoringError::other("test"));
        assert!(failure.is_err());
    }
}
