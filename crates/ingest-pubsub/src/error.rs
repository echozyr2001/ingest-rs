use thiserror::Error;

/// Result type for pub-sub operations
pub type Result<T> = std::result::Result<T, PubSubError>;

/// Errors that can occur in the pub-sub system
#[derive(Error, Debug)]
pub enum PubSubError {
    /// Topic-related errors
    #[error("Topic error: {message}")]
    Topic { message: String },

    /// Subscription-related errors
    #[error("Subscription error: {message}")]
    Subscription { message: String },

    /// Message-related errors
    #[error("Message error: {message}")]
    Message { message: String },

    /// Broker-related errors
    #[error("Broker error: {message}")]
    Broker { message: String },

    /// Storage-related errors
    #[error("Storage error: {source}")]
    Storage {
        #[from]
        source: ingest_storage::StorageError,
    },

    /// Serialization errors
    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Timeout errors
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String },

    /// Resource exhaustion errors
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    /// Invalid operation errors
    #[error("Invalid operation: {operation} - {reason}")]
    InvalidOperation { operation: String, reason: String },

    /// Internal errors
    #[error("Internal error: {message}")]
    Internal { message: String },

    /// Generic errors from other sources
    #[error("External error: {source}")]
    External {
        #[from]
        source: anyhow::Error,
    },
}

impl PubSubError {
    /// Create a new topic error
    pub fn topic(message: impl Into<String>) -> Self {
        Self::Topic {
            message: message.into(),
        }
    }

    /// Create a new subscription error
    pub fn subscription(message: impl Into<String>) -> Self {
        Self::Subscription {
            message: message.into(),
        }
    }

    /// Create a new message error
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }

    /// Create a new broker error
    pub fn broker(message: impl Into<String>) -> Self {
        Self::Broker {
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create a new resource exhausted error
    pub fn resource_exhausted(resource: impl Into<String>) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
        }
    }

    /// Create a new invalid operation error
    pub fn invalid_operation(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidOperation {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create a new internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Topic { .. } => false,
            Self::Subscription { .. } => false,
            Self::Message { .. } => false,
            Self::Broker { .. } => true,
            Self::Storage { .. } => true,
            Self::Serialization { .. } => false,
            Self::Configuration { .. } => false,
            Self::Timeout { .. } => true,
            Self::ResourceExhausted { .. } => true,
            Self::InvalidOperation { .. } => false,
            Self::Internal { .. } => true,
            Self::External { .. } => false,
        }
    }

    /// Get the error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            Self::Topic { .. } => "topic",
            Self::Subscription { .. } => "subscription",
            Self::Message { .. } => "message",
            Self::Broker { .. } => "broker",
            Self::Storage { .. } => "storage",
            Self::Serialization { .. } => "serialization",
            Self::Configuration { .. } => "configuration",
            Self::Timeout { .. } => "timeout",
            Self::ResourceExhausted { .. } => "resource_exhausted",
            Self::InvalidOperation { .. } => "invalid_operation",
            Self::Internal { .. } => "internal",
            Self::External { .. } => "external",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_topic_error_creation() {
        let fixture = "Topic not found";
        let actual = PubSubError::topic(fixture);

        match actual {
            PubSubError::Topic { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Topic error"),
        }
    }

    #[test]
    fn test_subscription_error_creation() {
        let fixture = "Subscription failed";
        let actual = PubSubError::subscription(fixture);

        match actual {
            PubSubError::Subscription { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Subscription error"),
        }
    }

    #[test]
    fn test_message_error_creation() {
        let fixture = "Message invalid";
        let actual = PubSubError::message(fixture);

        match actual {
            PubSubError::Message { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Message error"),
        }
    }

    #[test]
    fn test_broker_error_creation() {
        let fixture = "Broker unavailable";
        let actual = PubSubError::broker(fixture);

        match actual {
            PubSubError::Broker { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Broker error"),
        }
    }

    #[test]
    fn test_configuration_error_creation() {
        let fixture = "Invalid configuration";
        let actual = PubSubError::configuration(fixture);

        match actual {
            PubSubError::Configuration { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[test]
    fn test_timeout_error_creation() {
        let fixture = "publish_message";
        let actual = PubSubError::timeout(fixture);

        match actual {
            PubSubError::Timeout { operation } => assert_eq!(operation, fixture),
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_resource_exhausted_error_creation() {
        let fixture = "memory";
        let actual = PubSubError::resource_exhausted(fixture);

        match actual {
            PubSubError::ResourceExhausted { resource } => assert_eq!(resource, fixture),
            _ => panic!("Expected ResourceExhausted error"),
        }
    }

    #[test]
    fn test_invalid_operation_error_creation() {
        let operation_fixture = "subscribe";
        let reason_fixture = "topic does not exist";
        let actual = PubSubError::invalid_operation(operation_fixture, reason_fixture);

        match actual {
            PubSubError::InvalidOperation { operation, reason } => {
                assert_eq!(operation, operation_fixture);
                assert_eq!(reason, reason_fixture);
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_internal_error_creation() {
        let fixture = "Unexpected internal state";
        let actual = PubSubError::internal(fixture);

        match actual {
            PubSubError::Internal { message } => assert_eq!(message, fixture),
            _ => panic!("Expected Internal error"),
        }
    }

    #[test]
    fn test_error_retryability() {
        let retryable_errors = vec![
            PubSubError::broker("test"),
            PubSubError::timeout("test"),
            PubSubError::resource_exhausted("test"),
            PubSubError::internal("test"),
        ];

        for error in retryable_errors {
            assert!(
                error.is_retryable(),
                "Error should be retryable: {:?}",
                error
            );
        }

        let non_retryable_errors = vec![
            PubSubError::topic("test"),
            PubSubError::subscription("test"),
            PubSubError::message("test"),
            PubSubError::configuration("test"),
            PubSubError::invalid_operation("test", "test"),
        ];

        for error in non_retryable_errors {
            assert!(
                !error.is_retryable(),
                "Error should not be retryable: {:?}",
                error
            );
        }
    }

    #[test]
    fn test_error_categories() {
        let test_cases = vec![
            (PubSubError::topic("test"), "topic"),
            (PubSubError::subscription("test"), "subscription"),
            (PubSubError::message("test"), "message"),
            (PubSubError::broker("test"), "broker"),
            (PubSubError::configuration("test"), "configuration"),
            (PubSubError::timeout("test"), "timeout"),
            (
                PubSubError::resource_exhausted("test"),
                "resource_exhausted",
            ),
            (
                PubSubError::invalid_operation("test", "test"),
                "invalid_operation",
            ),
            (PubSubError::internal("test"), "internal"),
        ];

        for (error, expected_category) in test_cases {
            let actual = error.category();
            assert_eq!(actual, expected_category);
        }
    }

    #[test]
    fn test_error_display() {
        let fixture = PubSubError::topic("Topic not found");
        let actual = format!("{fixture}");
        let expected = "Topic error: Topic not found";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_debug() {
        let fixture = PubSubError::topic("Topic not found");
        let actual = format!("{fixture:?}");
        assert!(actual.contains("Topic"));
        assert!(actual.contains("Topic not found"));
    }
}
