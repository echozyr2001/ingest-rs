//! Error types for state management operations.

use inngest_core;
use thiserror::Error;

/// State management result type
pub type StateResult<T> = Result<T, StateError>;

/// Errors that can occur during state management operations
#[derive(Debug, Error)]
pub enum StateError {
    /// Redis connection or operation error
    #[error("Redis error: {0}")]
    Redis(String),

    /// Lua script execution error
    #[error("Lua script error: {script_name}: {message}")]
    LuaScript {
        script_name: String,
        message: String,
    },

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// State not found
    #[error("State not found for identifier: {identifier}")]
    StateNotFound { identifier: String },

    /// Run ID already exists
    #[error("Run ID already exists: {run_id}")]
    RunIdExists { run_id: String },

    /// Duplicate step response
    #[error("Duplicate step response for step: {step_id}")]
    DuplicateStepResponse { step_id: String },

    /// Step not found
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: String },

    /// Invalid step state
    #[error("Invalid step state: {message}")]
    InvalidStepState { message: String },

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout error
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl StateError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            StateError::Redis(_) => true,
            StateError::LuaScript { .. } => false,
            StateError::Serialization(_) => false,
            StateError::StateNotFound { .. } => false,
            StateError::RunIdExists { .. } => false,
            StateError::DuplicateStepResponse { .. } => false,
            StateError::StepNotFound { .. } => false,
            StateError::InvalidStepState { .. } => false,
            StateError::Config(_) => false,
            StateError::Connection(_) => true,
            StateError::Timeout { .. } => true,
            StateError::Internal(_) => false,
        }
    }

    /// Get error code for categorization
    pub fn code(&self) -> &'static str {
        match self {
            StateError::Redis(_) => "REDIS_ERROR",
            StateError::LuaScript { .. } => "LUA_SCRIPT_ERROR",
            StateError::Serialization(_) => "SERIALIZATION_ERROR",
            StateError::StateNotFound { .. } => "STATE_NOT_FOUND",
            StateError::RunIdExists { .. } => "RUN_ID_EXISTS",
            StateError::DuplicateStepResponse { .. } => "DUPLICATE_STEP_RESPONSE",
            StateError::StepNotFound { .. } => "STEP_NOT_FOUND",
            StateError::InvalidStepState { .. } => "INVALID_STEP_STATE",
            StateError::Config(_) => "CONFIG_ERROR",
            StateError::Connection(_) => "CONNECTION_ERROR",
            StateError::Timeout { .. } => "TIMEOUT_ERROR",
            StateError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Create a Redis error
    pub fn redis(msg: impl Into<String>) -> Self {
        StateError::Redis(msg.into())
    }

    /// Create a Lua script error
    pub fn lua_script(script_name: impl Into<String>, message: impl Into<String>) -> Self {
        StateError::LuaScript {
            script_name: script_name.into(),
            message: message.into(),
        }
    }

    /// Create a state not found error
    pub fn state_not_found(identifier: impl Into<String>) -> Self {
        StateError::StateNotFound {
            identifier: identifier.into(),
        }
    }

    /// Create a run ID exists error
    pub fn run_id_exists(run_id: impl Into<String>) -> Self {
        StateError::RunIdExists {
            run_id: run_id.into(),
        }
    }

    /// Create a config error
    pub fn config(msg: impl Into<String>) -> Self {
        StateError::Config(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        StateError::Internal(msg.into())
    }
}

// Conversion from fred::error::Error
impl From<fred::error::Error> for StateError {
    fn from(err: fred::error::Error) -> Self {
        StateError::Redis(err.to_string())
    }
}

// Conversion to inngest-core StateError for interoperability
impl From<StateError> for inngest_core::error::StateError {
    fn from(err: StateError) -> Self {
        match err {
            StateError::Redis(msg) => inngest_core::error::StateError::Connection { message: msg },
            StateError::LuaScript { message, .. } => {
                inngest_core::error::StateError::Internal { message }
            }
            StateError::Serialization(e) => inngest_core::error::StateError::Serialization {
                message: e.to_string(),
            },
            StateError::StateNotFound { identifier } => {
                inngest_core::error::StateError::NotFound { identifier }
            }
            StateError::RunIdExists { run_id } => {
                inngest_core::error::StateError::InvalidOperation {
                    message: format!("Run ID already exists: {run_id}"),
                }
            }
            StateError::DuplicateStepResponse { step_id } => {
                inngest_core::error::StateError::InvalidOperation {
                    message: format!("Duplicate step response for step: {step_id}"),
                }
            }
            StateError::StepNotFound { step_id } => inngest_core::error::StateError::NotFound {
                identifier: format!("step:{step_id}"),
            },
            StateError::InvalidStepState { message } => {
                inngest_core::error::StateError::InvalidOperation { message }
            }
            StateError::Config(msg) => inngest_core::error::StateError::Internal { message: msg },
            StateError::Connection(msg) => {
                inngest_core::error::StateError::Connection { message: msg }
            }
            StateError::Timeout { operation } => {
                inngest_core::error::StateError::Timeout { operation }
            }
            StateError::Internal(msg) => inngest_core::error::StateError::Internal { message: msg },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_creation() {
        let err = StateError::redis("connection failed");
        assert_eq!(err.code(), "REDIS_ERROR");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_lua_script_error() {
        let err = StateError::lua_script("new.lua", "invalid argument");
        assert_eq!(err.code(), "LUA_SCRIPT_ERROR");
        assert!(!err.is_retryable());
        assert_eq!(
            err.to_string(),
            "Lua script error: new.lua: invalid argument"
        );
    }

    #[test]
    fn test_state_not_found_error() {
        let err = StateError::state_not_found("test-run-id");
        assert_eq!(err.code(), "STATE_NOT_FOUND");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_error_categorization() {
        let retryable_errors = vec![
            StateError::redis("test"),
            StateError::Connection("test".to_string()),
            StateError::Timeout {
                operation: "test".to_string(),
            },
        ];

        let non_retryable_errors = vec![
            StateError::lua_script("test", "test"),
            StateError::state_not_found("test"),
            StateError::config("test"),
        ];

        for err in retryable_errors {
            assert!(err.is_retryable(), "Expected {err} to be retryable");
        }

        for err in non_retryable_errors {
            assert!(!err.is_retryable(), "Expected {err} to not be retryable");
        }
    }
}
