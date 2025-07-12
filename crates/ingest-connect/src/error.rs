//! Error types for the ingest-connect crate

use thiserror::Error;

/// Result type alias for the connect crate
pub type Result<T> = std::result::Result<T, ConnectError>;

/// Errors that can occur in the connect system
#[derive(Debug, Error)]
pub enum ConnectError {
    /// Authentication errors
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Authorization errors
    #[error("Authorization failed: {0}")]
    Authorization(String),

    /// Protocol negotiation errors
    #[error("Protocol negotiation failed: {0}")]
    ProtocolNegotiation(String),

    /// WebSocket connection errors
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Connection pool errors
    #[error("Connection pool error: {0}")]
    ConnectionPool(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Network errors
    #[error("Network error: {0}")]
    Network(String),

    /// Timeout errors
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Connection not found
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Rate limiting errors
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Too many connections
    #[error("Too many connections")]
    TooManyConnections,

    /// Internal server errors
    #[error("Internal server error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for ConnectError {
    fn from(error: serde_json::Error) -> Self {
        ConnectError::Serialization(error.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ConnectError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        ConnectError::WebSocket(error.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for ConnectError {
    fn from(error: jsonwebtoken::errors::Error) -> Self {
        ConnectError::Authentication(error.to_string())
    }
}

impl From<anyhow::Error> for ConnectError {
    fn from(error: anyhow::Error) -> Self {
        ConnectError::Internal(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_display() {
        let error = ConnectError::Authentication("invalid token".to_string());
        assert_eq!(error.to_string(), "Authentication failed: invalid token");
    }

    #[test]
    fn test_error_conversion_from_serde() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_error.is_err());

        let connect_error: ConnectError = json_error.unwrap_err().into();
        assert!(matches!(connect_error, ConnectError::Serialization(_)));
    }

    #[test]
    fn test_error_conversion_from_anyhow() {
        let anyhow_error = anyhow::anyhow!("test error");
        let connect_error: ConnectError = anyhow_error.into();
        assert!(matches!(connect_error, ConnectError::Internal(_)));
    }

    #[test]
    fn test_result_type() {
        let success: Result<String> = Ok("test".to_string());
        assert!(success.is_ok());

        let failure: Result<String> = Err(ConnectError::Network("connection failed".to_string()));
        assert!(failure.is_err());
    }
}
