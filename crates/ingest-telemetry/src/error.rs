use thiserror::Error;

/// Telemetry error types
#[derive(Error, Debug)]
pub enum TelemetryError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Initialization error
    #[error("Initialization error: {0}")]
    Initialization(String),

    /// Export error
    #[error("Export error: {0}")]
    Export(String),

    /// Metrics error
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Tracing error
    #[error("Tracing error: {0}")]
    Tracing(String),

    /// OpenTelemetry error
    #[error("OpenTelemetry error: {0}")]
    OpenTelemetry(String),

    /// Prometheus error
    #[error("Prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Telemetry error: {0}")]
    Other(String),
}

impl TelemetryError {
    /// Create a configuration error
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    /// Create an initialization error
    pub fn initialization(msg: impl Into<String>) -> Self {
        Self::Initialization(msg.into())
    }

    /// Create an export error
    pub fn export(msg: impl Into<String>) -> Self {
        Self::Export(msg.into())
    }

    /// Create a metrics error
    pub fn metrics(msg: impl Into<String>) -> Self {
        Self::Metrics(msg.into())
    }

    /// Create a tracing error
    pub fn tracing(msg: impl Into<String>) -> Self {
        Self::Tracing(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Result type for telemetry operations
pub type Result<T> = std::result::Result<T, TelemetryError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_telemetry_error_configuration() {
        let error = TelemetryError::configuration("test error");
        assert_eq!(error.to_string(), "Configuration error: test error");
    }

    #[test]
    fn test_telemetry_error_initialization() {
        let error = TelemetryError::initialization("init failed");
        assert_eq!(error.to_string(), "Initialization error: init failed");
    }

    #[test]
    fn test_telemetry_error_export() {
        let error = TelemetryError::export("export failed");
        assert_eq!(error.to_string(), "Export error: export failed");
    }

    #[test]
    fn test_telemetry_error_metrics() {
        let error = TelemetryError::metrics("metrics failed");
        assert_eq!(error.to_string(), "Metrics error: metrics failed");
    }

    #[test]
    fn test_telemetry_error_tracing() {
        let error = TelemetryError::tracing("tracing failed");
        assert_eq!(error.to_string(), "Tracing error: tracing failed");
    }

    #[test]
    fn test_telemetry_error_other() {
        let error = TelemetryError::other("generic error");
        assert_eq!(error.to_string(), "Telemetry error: generic error");
    }

    #[test]
    fn test_result_type() {
        let failure: Result<i32> = Err(TelemetryError::other("test"));
        assert!(failure.is_err());
    }
}
