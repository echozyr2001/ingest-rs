//! Error types and result handling for Inngest

use thiserror::Error;

/// Result type alias for Inngest operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Inngest operations
#[derive(Debug, Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("State management error: {0}")]
    State(#[from] StateError),

    #[error("Queue operation error: {0}")]
    Queue(#[from] QueueError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("Connect protocol error: {0}")]
    Connect(#[from] ConnectError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing required configuration: {key}")]
    MissingRequired { key: String },

    #[error("Invalid configuration value for {key}: {value}")]
    InvalidValue { key: String, value: String },

    #[error("Configuration file error: {0}")]
    File(#[from] std::io::Error),
}

/// Generic state management errors - high-level abstraction
/// Specific implementations can convert their detailed errors to these variants
#[derive(Debug, Error)]
pub enum StateError {
    #[error("State not found: {identifier}")]
    NotFound { identifier: String },

    #[error("State already exists")]
    AlreadyExists,

    #[error("Invalid state operation: {message}")]
    InvalidOperation { message: String },

    #[error("State too large: {size} bytes")]
    TooLarge { size: usize },

    #[error("Concurrent modification detected")]
    ConcurrentModification,

    #[error("Connection error: {message}")]
    Connection { message: String },

    #[error("Operation timeout: {operation}")]
    Timeout { operation: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl StateError {
    /// Get error code for categorization
    pub fn code(&self) -> &'static str {
        match self {
            StateError::NotFound { .. } => "STATE_NOT_FOUND",
            StateError::AlreadyExists => "STATE_ALREADY_EXISTS",
            StateError::InvalidOperation { .. } => "INVALID_STATE_OPERATION",
            StateError::TooLarge { .. } => "STATE_TOO_LARGE",
            StateError::ConcurrentModification => "CONCURRENT_MODIFICATION",
            StateError::Connection { .. } => "CONNECTION_ERROR",
            StateError::Timeout { .. } => "TIMEOUT_ERROR",
            StateError::Serialization { .. } => "SERIALIZATION_ERROR",
            StateError::Internal { .. } => "INTERNAL_ERROR",
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StateError::Connection { .. }
                | StateError::Timeout { .. }
                | StateError::ConcurrentModification
        )
    }
}

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("Queue is full")]
    Full,

    #[error("Queue item not found: {id}")]
    ItemNotFound { id: String },

    #[error("Invalid queue configuration")]
    InvalidConfiguration,

    #[error("Lease expired for item: {id}")]
    LeaseExpired { id: String },

    #[error("Concurrency limit exceeded")]
    ConcurrencyLimitExceeded,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Function not found: {id}")]
    FunctionNotFound { id: String },

    #[error("Execution timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Step failed: {step_id}")]
    StepFailed { step_id: String },

    #[error("Invalid step configuration")]
    InvalidStepConfiguration,

    #[error("Driver error: {driver}")]
    Driver { driver: String },

    #[error("Retry limit exceeded")]
    RetryLimitExceeded,
}

#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("Connection not found: {id}")]
    ConnectionNotFound { id: String },

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Protocol violation: {message}")]
    ProtocolViolation { message: String },

    #[error("Message too large: {size} bytes")]
    MessageTooLarge { size: usize },

    #[error("Gateway unavailable")]
    GatewayUnavailable,
}

impl Error {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Queue(QueueError::ConcurrencyLimitExceeded)
                | Error::Queue(QueueError::RateLimitExceeded)
                | Error::State(StateError::Connection { .. })
                | Error::State(StateError::Timeout { .. })
                | Error::State(StateError::ConcurrentModification)
                | Error::Execution(ExecutionError::Timeout { .. })
                | Error::Connect(ConnectError::GatewayUnavailable)
                | Error::Database(_)
                | Error::Redis(_)
                | Error::Http(_)
                | Error::Io(_)
        )
    }

    /// Get the error code for external APIs
    pub fn code(&self) -> &'static str {
        match self {
            Error::Config(_) => "CONFIG_ERROR",
            Error::State(state_error) => state_error.code(),
            Error::Queue(QueueError::Full) => "QUEUE_FULL",
            Error::Queue(QueueError::ItemNotFound { .. }) => "QUEUE_ITEM_NOT_FOUND",
            Error::Queue(QueueError::ConcurrencyLimitExceeded) => "CONCURRENCY_LIMIT_EXCEEDED",
            Error::Queue(QueueError::RateLimitExceeded) => "RATE_LIMIT_EXCEEDED",
            Error::Execution(ExecutionError::FunctionNotFound { .. }) => "FUNCTION_NOT_FOUND",
            Error::Execution(ExecutionError::Timeout { .. }) => "EXECUTION_TIMEOUT",
            Error::Connect(ConnectError::AuthenticationFailed) => "AUTHENTICATION_FAILED",
            Error::Connect(ConnectError::GatewayUnavailable) => "GATEWAY_UNAVAILABLE",
            Error::NotFound { .. } => "NOT_FOUND",
            Error::Conflict { .. } => "CONFLICT",
            Error::Timeout { .. } => "TIMEOUT",
            Error::InvalidInput { .. } => "INVALID_INPUT",
            _ => "INTERNAL_ERROR",
        }
    }
}

/// Convenience macros for creating errors
#[macro_export]
macro_rules! invalid_input {
    ($msg:expr) => {
        $crate::Error::InvalidInput {
            message: $msg.to_string(),
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::Error::InvalidInput {
            message: format!($fmt, $($arg)*),
        }
    };
}

#[macro_export]
macro_rules! not_found {
    ($resource:expr) => {
        $crate::Error::NotFound {
            resource: $resource.to_string(),
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::Error::NotFound {
            resource: format!($fmt, $($arg)*),
        }
    };
}

#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        $crate::Error::Internal {
            message: $msg.to_string(),
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::Error::Internal {
            message: format!($fmt, $($arg)*),
        }
    };
}
