//! Error types for event processing

use thiserror::Error;

/// Event processing errors
#[derive(Error, Debug)]
pub enum EventError {
    /// Event validation failed
    #[error("Event validation failed: {message}")]
    Validation { message: String },

    /// Event serialization failed
    #[error("Event serialization failed: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// Event routing failed
    #[error("Event routing failed: {message}")]
    Routing { message: String },

    /// Event processing failed
    #[error("Event processing failed: {message}")]
    Processing { message: String },

    /// Pattern matching failed
    #[error("Pattern matching failed: {pattern}")]
    PatternMatch { pattern: String },

    /// Schema validation failed
    #[error("Schema validation failed: {errors:?}")]
    Schema { errors: Vec<String> },

    /// Buffer overflow
    #[error("Buffer overflow: capacity {capacity} exceeded")]
    BufferOverflow { capacity: usize },

    /// Stream error
    #[error("Stream error: {message}")]
    Stream { message: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Core error
    #[error("Core error: {source}")]
    Core {
        #[from]
        source: ingest_core::Error,
    },

    /// Storage error
    #[error("Storage error: {source}")]
    Storage {
        #[from]
        source: anyhow::Error,
    },
}

impl EventError {
    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a routing error
    pub fn routing(message: impl Into<String>) -> Self {
        Self::Routing {
            message: message.into(),
        }
    }

    /// Create a processing error
    pub fn processing(message: impl Into<String>) -> Self {
        Self::Processing {
            message: message.into(),
        }
    }

    /// Create a pattern match error
    pub fn pattern_match(pattern: impl Into<String>) -> Self {
        Self::PatternMatch {
            pattern: pattern.into(),
        }
    }

    /// Create a schema validation error
    pub fn schema(errors: Vec<String>) -> Self {
        Self::Schema { errors }
    }

    /// Create a buffer overflow error
    pub fn buffer_overflow(capacity: usize) -> Self {
        Self::BufferOverflow { capacity }
    }

    /// Create a stream error
    pub fn stream(message: impl Into<String>) -> Self {
        Self::Stream {
            message: message.into(),
        }
    }

    /// Create a config error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
}

/// Result type for event operations
pub type Result<T> = std::result::Result<T, EventError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_validation_error() {
        let fixture = "Invalid event name";
        let actual = EventError::validation(fixture);
        let expected = "Event validation failed: Invalid event name";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_routing_error() {
        let fixture = "No matching route";
        let actual = EventError::routing(fixture);
        let expected = "Event routing failed: No matching route";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_processing_error() {
        let fixture = "Handler failed";
        let actual = EventError::processing(fixture);
        let expected = "Event processing failed: Handler failed";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_pattern_match_error() {
        let fixture = "user.*";
        let actual = EventError::pattern_match(fixture);
        let expected = "Pattern matching failed: user.*";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_schema_error() {
        let fixture = vec![
            "Missing field: name".to_string(),
            "Invalid type: age".to_string(),
        ];
        let actual = EventError::schema(fixture);
        assert!(actual.to_string().contains("Schema validation failed"));
    }

    #[test]
    fn test_buffer_overflow_error() {
        let fixture = 1000;
        let actual = EventError::buffer_overflow(fixture);
        let expected = "Buffer overflow: capacity 1000 exceeded";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_stream_error() {
        let fixture = "Stream closed";
        let actual = EventError::stream(fixture);
        let expected = "Stream error: Stream closed";
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn test_config_error() {
        let fixture = "Invalid configuration";
        let actual = EventError::config(fixture);
        let expected = "Configuration error: Invalid configuration";
        assert_eq!(actual.to_string(), expected);
    }
}
