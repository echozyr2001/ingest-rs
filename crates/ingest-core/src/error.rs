use thiserror::Error;

/// Core error types for the Inngest platform
#[derive(Error, Debug)]
pub enum Error {
    #[error("Event error: {message}")]
    Event { message: String },

    #[error("Function error: {message}")]
    Function { message: String },

    #[error("Step error: {message}")]
    Step { message: String },

    #[error("State error: {message}")]
    State { message: String },

    #[error("Queue error: {message}")]
    Queue { message: String },

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    #[error("UUID error: {source}")]
    Uuid {
        #[from]
        source: uuid::Error,
    },

    #[error("URL error: {source}")]
    Url {
        #[from]
        source: url::ParseError,
    },

    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Generic error: {message}")]
    Generic { message: String },
}

impl Error {
    /// Create a new event error
    pub fn event(message: impl Into<String>) -> Self {
        Self::Event {
            message: message.into(),
        }
    }

    /// Create a new function error
    pub fn function(message: impl Into<String>) -> Self {
        Self::Function {
            message: message.into(),
        }
    }

    /// Create a new step error
    pub fn step(message: impl Into<String>) -> Self {
        Self::Step {
            message: message.into(),
        }
    }

    /// Create a new state error
    pub fn state(message: impl Into<String>) -> Self {
        Self::State {
            message: message.into(),
        }
    }

    /// Create a new queue error
    pub fn queue(message: impl Into<String>) -> Self {
        Self::Queue {
            message: message.into(),
        }
    }

    /// Create a new storage error
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a new generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_creation() {
        let fixture = "test error message";
        let actual = Error::event(fixture);
        let expected = Error::Event {
            message: "test error message".to_string(),
        };
        assert_eq!(format!("{}", actual), format!("{}", expected));
    }

    #[test]
    fn test_error_from_serde() {
        let fixture = serde_json::from_str::<serde_json::Value>("invalid json");
        let actual = Error::from(fixture.unwrap_err());
        assert!(matches!(actual, Error::Serialization { .. }));
    }
}
