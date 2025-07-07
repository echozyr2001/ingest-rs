use thiserror::Error;

/// State management error types
#[derive(Error, Debug)]
pub enum StateError {
    /// State not found
    #[error("State not found for run ID: {run_id}")]
    StateNotFound { run_id: String },

    /// Version conflict during optimistic locking
    #[error("Version conflict for run ID {run_id}: expected {expected}, got {actual}")]
    VersionConflict {
        run_id: String,
        expected: u64,
        actual: u64,
    },

    /// Invalid state transition
    #[error("Invalid state transition from {from} to {to}: {reason}")]
    InvalidTransition {
        from: String,
        to: String,
        reason: String,
    },

    /// Snapshot not found
    #[error("Snapshot not found: {snapshot_id}")]
    SnapshotNotFound { snapshot_id: String },

    /// Invalid snapshot data
    #[error("Invalid snapshot data: {reason}")]
    InvalidSnapshot { reason: String },

    /// State validation error
    #[error("State validation failed: {reason}")]
    ValidationError { reason: String },

    /// Storage operation failed
    #[error("Storage operation failed: {operation}")]
    StorageError { operation: String },

    /// Serialization error
    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    /// Database error
    #[error("Database error: {message}")]
    DatabaseError { message: String },

    /// Cache error
    #[error("Cache error: {message}")]
    CacheError { message: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// Timeout error
    #[error("Operation timed out: {operation}")]
    TimeoutError { operation: String },

    /// Concurrency error
    #[error("Concurrency error: {message}")]
    ConcurrencyError { message: String },

    /// Recovery error
    #[error("Recovery failed: {reason}")]
    RecoveryError { reason: String },

    /// Not implemented error
    #[error("Feature not implemented: {feature}")]
    NotImplemented { feature: String },

    /// Optimistic lock failure
    #[error("Optimistic lock failure: {message}")]
    OptimisticLockFailure { message: String },
}

impl StateError {
    /// Create a state not found error
    pub fn state_not_found(run_id: impl Into<String>) -> Self {
        Self::StateNotFound {
            run_id: run_id.into(),
        }
    }

    /// Create a version conflict error
    pub fn version_conflict(run_id: impl Into<String>, expected: u64, actual: u64) -> Self {
        Self::VersionConflict {
            run_id: run_id.into(),
            expected,
            actual,
        }
    }

    /// Create an invalid transition error
    pub fn invalid_transition(
        from: impl Into<String>,
        to: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidTransition {
            from: from.into(),
            to: to.into(),
            reason: reason.into(),
        }
    }

    /// Create a snapshot not found error
    pub fn snapshot_not_found(snapshot_id: impl Into<String>) -> Self {
        Self::SnapshotNotFound {
            snapshot_id: snapshot_id.into(),
        }
    }

    /// Create an invalid snapshot error
    pub fn invalid_snapshot(reason: impl Into<String>) -> Self {
        Self::InvalidSnapshot {
            reason: reason.into(),
        }
    }

    /// Create a validation error
    pub fn validation(reason: impl Into<String>) -> Self {
        Self::ValidationError {
            reason: reason.into(),
        }
    }

    /// Create a storage error
    pub fn storage(operation: impl Into<String>) -> Self {
        Self::StorageError {
            operation: operation.into(),
        }
    }

    /// Create a serialization error
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::SerializationError {
            message: message.into(),
        }
    }

    /// Create a database error
    pub fn database(message: impl Into<String>) -> Self {
        Self::DatabaseError {
            message: message.into(),
        }
    }

    /// Create an invalid status error
    pub fn invalid_status(message: impl Into<String>) -> Self {
        Self::ValidationError {
            reason: message.into(),
        }
    }

    /// Create a cache error
    pub fn cache(message: impl Into<String>) -> Self {
        Self::CacheError {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::TimeoutError {
            operation: operation.into(),
        }
    }

    /// Create a concurrency error
    pub fn concurrency(message: impl Into<String>) -> Self {
        Self::ConcurrencyError {
            message: message.into(),
        }
    }

    /// Create a recovery error
    pub fn recovery(reason: impl Into<String>) -> Self {
        Self::RecoveryError {
            reason: reason.into(),
        }
    }

    /// Create a not implemented error
    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented {
            feature: feature.into(),
        }
    }

    /// Create an optimistic lock failure error
    pub fn optimistic_lock_failure(message: impl Into<String>) -> Self {
        Self::OptimisticLockFailure {
            message: message.into(),
        }
    }
}

/// Result type for state operations
pub type Result<T> = std::result::Result<T, StateError>;

/// Convert from sqlx errors
impl From<sqlx::Error> for StateError {
    fn from(err: sqlx::Error) -> Self {
        StateError::database(err.to_string())
    }
}

/// Convert from redis errors
impl From<redis::RedisError> for StateError {
    fn from(err: redis::RedisError) -> Self {
        StateError::cache(err.to_string())
    }
}

/// Convert from serde_json errors
impl From<serde_json::Error> for StateError {
    fn from(err: serde_json::Error) -> Self {
        StateError::serialization(err.to_string())
    }
}

/// Convert from ingest-storage errors
impl From<ingest_storage::StorageError> for StateError {
    fn from(err: ingest_storage::StorageError) -> Self {
        StateError::storage(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_state_not_found_error() {
        let fixture = "run_123";
        let actual = StateError::state_not_found(fixture);

        match actual {
            StateError::StateNotFound { run_id } => {
                assert_eq!(run_id, "run_123");
            }
            _ => panic!("Expected StateNotFound error"),
        }
    }

    #[test]
    fn test_version_conflict_error() {
        let fixture_run_id = "run_123";
        let fixture_expected = 5;
        let fixture_actual = 3;

        let actual = StateError::version_conflict(fixture_run_id, fixture_expected, fixture_actual);

        match actual {
            StateError::VersionConflict {
                run_id,
                expected,
                actual,
            } => {
                assert_eq!(run_id, "run_123");
                assert_eq!(expected, 5);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected VersionConflict error"),
        }
    }

    #[test]
    fn test_invalid_transition_error() {
        let fixture_from = "running";
        let fixture_to = "pending";
        let fixture_reason = "Cannot go backwards";

        let actual = StateError::invalid_transition(fixture_from, fixture_to, fixture_reason);

        match actual {
            StateError::InvalidTransition { from, to, reason } => {
                assert_eq!(from, "running");
                assert_eq!(to, "pending");
                assert_eq!(reason, "Cannot go backwards");
            }
            _ => panic!("Expected InvalidTransition error"),
        }
    }

    #[test]
    fn test_error_display() {
        let fixture = StateError::state_not_found("test_run");
        let actual = format!("{fixture}");
        let expected = "State not found for run ID: test_run";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_debug() {
        let fixture = StateError::validation("Invalid data");
        let actual = format!("{fixture:?}");
        assert!(actual.contains("ValidationError"));
        assert!(actual.contains("Invalid data"));
    }

    #[test]
    fn test_from_sqlx_error() {
        let fixture = sqlx::Error::RowNotFound;
        let actual = StateError::from(fixture);

        match actual {
            StateError::DatabaseError { .. } => {}
            _ => panic!("Expected DatabaseError"),
        }
    }

    #[test]
    fn test_from_serde_json_error() {
        let fixture = serde_json::from_str::<serde_json::Value>("invalid json");
        let error = fixture.unwrap_err();
        let actual = StateError::from(error);

        match actual {
            StateError::SerializationError { .. } => {}
            _ => panic!("Expected SerializationError"),
        }
    }
}
