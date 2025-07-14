//! Error types for the Inngest History system

use thiserror::Error;

/// Result type alias for history operations
pub type Result<T> = std::result::Result<T, HistoryError>;

/// Main error type for history operations
#[derive(Error, Debug)]
pub enum HistoryError {
    /// Storage errors
    #[error("Storage error: {0}")]
    Storage(#[from] ingest_storage::StorageError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// SQL database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Query validation errors
    #[error("Query validation error: {field}: {message}")]
    QueryValidation { field: String, message: String },

    /// Retention policy errors
    #[error("Retention policy error: {message}")]
    RetentionPolicy { message: String },

    /// Archive operation errors
    #[error("Archive error: {message}")]
    Archive { message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Not found errors
    #[error("Not found: {resource_type} with ID {id}")]
    NotFound { resource_type: String, id: String },

    /// Permission errors
    #[error("Permission denied: {action} on {resource_type}")]
    PermissionDenied {
        action: String,
        resource_type: String,
    },

    /// Concurrent modification errors
    #[error("Concurrent modification: {resource_type} {id} was modified")]
    ConcurrentModification { resource_type: String, id: String },

    /// Generic errors
    #[error("History error: {message}")]
    Generic { message: String },

    /// Core library errors
    #[error("Core error: {0}")]
    Core(#[from] ingest_core::Error),
}

impl HistoryError {
    /// Create a query validation error
    pub fn query_validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::QueryValidation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a retention policy error
    pub fn retention_policy(message: impl Into<String>) -> Self {
        Self::RetentionPolicy {
            message: message.into(),
        }
    }

    /// Create an archive error
    pub fn archive(message: impl Into<String>) -> Self {
        Self::Archive {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a not found error
    pub fn not_found(resource_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            id: id.into(),
        }
    }

    /// Create a permission denied error
    pub fn permission_denied(action: impl Into<String>, resource_type: impl Into<String>) -> Self {
        Self::PermissionDenied {
            action: action.into(),
            resource_type: resource_type.into(),
        }
    }

    /// Create a concurrent modification error
    pub fn concurrent_modification(
        resource_type: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        Self::ConcurrentModification {
            resource_type: resource_type.into(),
            id: id.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            HistoryError::Storage(err) => err.to_string().contains("connection"),
            HistoryError::Database(sqlx::Error::Io(_)) => true,
            HistoryError::Database(sqlx::Error::PoolTimedOut) => true,
            HistoryError::ConcurrentModification { .. } => true,
            _ => false,
        }
    }

    /// Get the error category for telemetry
    pub fn category(&self) -> &'static str {
        match self {
            HistoryError::Storage(_) => "storage",
            HistoryError::Serialization(_) => "serialization",
            HistoryError::Database(_) => "database",
            HistoryError::QueryValidation { .. } => "query_validation",
            HistoryError::RetentionPolicy { .. } => "retention_policy",
            HistoryError::Archive { .. } => "archive",
            HistoryError::Configuration { .. } => "configuration",
            HistoryError::NotFound { .. } => "not_found",
            HistoryError::PermissionDenied { .. } => "permission_denied",
            HistoryError::ConcurrentModification { .. } => "concurrent_modification",
            HistoryError::Generic { .. } => "generic",
            HistoryError::Core(_) => "core",
        }
    }

    /// Get the HTTP status code for API responses
    pub fn http_status(&self) -> u16 {
        match self {
            HistoryError::NotFound { .. } => 404,
            HistoryError::PermissionDenied { .. } => 403,
            HistoryError::QueryValidation { .. } => 400,
            HistoryError::Configuration { .. } => 400,
            HistoryError::ConcurrentModification { .. } => 409,
            HistoryError::Storage(_) => 500,
            HistoryError::Database(_) => 500,
            HistoryError::Serialization(_) => 500,
            HistoryError::RetentionPolicy { .. } => 500,
            HistoryError::Archive { .. } => 500,
            HistoryError::Generic { .. } => 500,
            HistoryError::Core(_) => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_query_validation_error() {
        let fixture = HistoryError::query_validation("limit", "Must be positive");
        let actual = fixture.to_string();
        let expected = "Query validation error: limit: Must be positive";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_retention_policy_error() {
        let fixture = HistoryError::retention_policy("Invalid retention period");
        let actual = fixture.to_string();
        let expected = "Retention policy error: Invalid retention period";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_not_found_error() {
        let fixture = HistoryError::not_found("function", "func_123");
        let actual = fixture.to_string();
        let expected = "Not found: function with ID func_123";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_permission_denied_error() {
        let fixture = HistoryError::permission_denied("delete", "audit_event");
        let actual = fixture.to_string();
        let expected = "Permission denied: delete on audit_event";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_concurrent_modification_error() {
        let fixture = HistoryError::concurrent_modification("policy", "policy_123");
        let actual = fixture.to_string();
        let expected = "Concurrent modification: policy policy_123 was modified";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_retryable() {
        let fixture = HistoryError::concurrent_modification("policy", "policy_123");
        assert!(fixture.is_retryable());

        let fixture = HistoryError::not_found("function", "func_123");
        assert!(!fixture.is_retryable());
    }

    #[test]
    fn test_error_category() {
        let fixture = HistoryError::query_validation("limit", "Invalid");
        let actual = fixture.category();
        let expected = "query_validation";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_http_status() {
        let fixture = HistoryError::not_found("function", "func_123");
        let actual = fixture.http_status();
        let expected = 404;
        assert_eq!(actual, expected);

        let fixture = HistoryError::permission_denied("delete", "audit_event");
        let actual = fixture.http_status();
        let expected = 403;
        assert_eq!(actual, expected);
    }
}
