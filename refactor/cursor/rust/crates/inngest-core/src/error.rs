//! Error types for Inngest
//!
//! This module defines all the error types used throughout the Inngest system,
//! providing a unified error handling approach.

use thiserror::Error;

/// The main error type for Inngest operations
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Event-related errors
    #[error("Event error: {0}")]
    Event(#[from] EventError),

    /// Function execution errors
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    /// State management errors
    #[error("State error: {0}")]
    State(#[from] StateError),

    /// Queue-related errors
    #[error("Queue error: {0}")]
    Queue(#[from] QueueError),

    /// Network/HTTP errors
    #[error("Network error: {0}")]
    Network(String),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Database errors
    #[error("Database error: {0}")]
    Database(String),

    /// Redis errors
    #[error("Redis error: {0}")]
    Redis(String),

    /// Generic internal errors
    #[error("Internal error: {0}")]
    Internal(String),

    /// Invalid data error
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Invalid configuration error
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Concurrency limit error
    #[error("Concurrency limit: {0}")]
    ConcurrencyLimit(String),

    /// Invalid input or parameters
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Operation not permitted
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Resource already exists
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// Timeout errors
    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Failed to load config: {0}")]
    LoadError(String),

    #[error("Validation failed: {0}")]
    Validation(String),
}

/// Event-related errors
#[derive(Error, Debug)]
pub enum EventError {
    #[error("Invalid event format: {0}")]
    InvalidFormat(String),

    #[error("Event validation failed: {0}")]
    Validation(String),

    #[error("Event too large: {size} bytes (max: {max} bytes)")]
    TooLarge { size: usize, max: usize },

    #[error("Invalid event name: {0}")]
    InvalidName(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Function execution errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Step execution failed: {step} - {error}")]
    StepFailed { step: String, error: String },

    #[error("Function timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Maximum retries exceeded: {attempts}")]
    MaxRetriesExceeded { attempts: u32 },

    #[error("Function cancelled: {reason}")]
    Cancelled { reason: String },

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Driver error: {0}")]
    Driver(String),
}

/// State management errors
#[derive(Error, Debug)]
pub enum StateError {
    #[error("State not found: {run_id}")]
    NotFound { run_id: String },

    #[error("Invalid state identifier: {0}")]
    InvalidIdentifier(String),

    #[error("State conflict: {0}")]
    Conflict(String),

    #[error("State corruption detected: {0}")]
    Corruption(String),

    #[error("State size limit exceeded: {size} bytes")]
    SizeLimit { size: usize },

    #[error("Idempotency key already exists: {key}")]
    IdempotencyExists { key: String },
}

/// Queue-related errors
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Queue item not found: {0}")]
    ItemNotFound(String),

    #[error("Queue shard unavailable: {0}")]
    ShardUnavailable(String),

    #[error("Queue full: {0}")]
    QueueFull(String),

    #[error("Invalid queue item: {0}")]
    InvalidItem(String),

    #[error("Lease expired: {0}")]
    LeaseExpired(String),
}

/// Network/HTTP errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("HTTP request failed: {status} - {message}")]
    Http { status: u16, message: String },

    #[error("Connection timeout")]
    Timeout,

    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Request body too large: {size} bytes")]
    RequestTooLarge { size: usize },
}

/// Convenience result type
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Network(_) => true, // Network errors are generally retryable
            Error::Redis(_) => true,
            Error::Database(_) => true,
            Error::Queue(QueueError::ShardUnavailable(_)) => true,
            Error::Execution(ExecutionError::Timeout { .. }) => true,
            Error::Timeout(_) => true,
            _ => false,
        }
    }

    /// Check if the error should be reported to external monitoring
    pub fn should_report(&self) -> bool {
        match self {
            Error::Internal(_) => true,
            Error::Database(_) => true,
            Error::Redis(_) => true,
            Error::Execution(ExecutionError::Runtime(_)) => true,
            _ => false,
        }
    }

    /// Get error code for structured logging
    pub fn error_code(&self) -> &'static str {
        match self {
            Error::Config(_) => "CONFIG_ERROR",
            Error::Event(_) => "EVENT_ERROR",
            Error::Execution(_) => "EXECUTION_ERROR",
            Error::State(_) => "STATE_ERROR",
            Error::Queue(_) => "QUEUE_ERROR",
            Error::Network(_) => "NETWORK_ERROR",
            Error::Serialization(_) => "SERIALIZATION_ERROR",
            Error::Database(_) => "DATABASE_ERROR",
            Error::Redis(_) => "REDIS_ERROR",
            Error::Internal(_) => "INTERNAL_ERROR",
            Error::InvalidData(_) => "INVALID_DATA",
            Error::InvalidConfiguration(_) => "INVALID_CONFIGURATION",
            Error::ConcurrencyLimit(_) => "CONCURRENCY_LIMIT",
            Error::InvalidInput(_) => "INVALID_INPUT",
            Error::NotFound(_) => "NOT_FOUND",
            Error::PermissionDenied(_) => "PERMISSION_DENIED",
            Error::AlreadyExists(_) => "ALREADY_EXISTS",
            Error::Timeout(_) => "TIMEOUT",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_properties() {
        let timeout_error = Error::Timeout("test timeout".to_string());
        assert!(timeout_error.is_retryable());
        assert_eq!(timeout_error.error_code(), "TIMEOUT");

        let config_error = Error::Config(ConfigError::Invalid("test".to_string()));
        assert!(!config_error.is_retryable());
        assert_eq!(config_error.error_code(), "CONFIG_ERROR");
    }

    #[test]
    fn test_error_chain() {
        let event_error = EventError::InvalidFormat("malformed JSON".to_string());
        let wrapped_error = Error::Event(event_error);

        assert_eq!(wrapped_error.error_code(), "EVENT_ERROR");
        assert!(!wrapped_error.is_retryable());
    }
}
