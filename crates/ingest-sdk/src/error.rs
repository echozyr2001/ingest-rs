//! Error types for the Inngest SDK

use thiserror::Error;

/// Result type alias for SDK operations
pub type Result<T> = std::result::Result<T, SdkError>;

/// Main error type for SDK operations
#[derive(Error, Debug)]
pub enum SdkError {
    /// HTTP client errors
    #[error("HTTP client error: {0}")]
    Http(String),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// URL parsing errors
    #[error("URL parsing error: {0}")]
    UrlParsing(String),

    /// Authentication errors
    #[error("Authentication error: {message}")]
    Authentication { message: String },

    /// API errors from the server
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Function execution errors
    #[error("Function execution error: {message}")]
    Execution { message: String },

    /// Local server errors
    #[error("Local server error: {message}")]
    LocalServer { message: String },

    /// Validation errors
    #[error("Validation error: {field}: {message}")]
    Validation { field: String, message: String },

    /// Generic errors
    #[error("SDK error: {message}")]
    Generic { message: String },

    /// Core library errors
    #[error("Core error: {0}")]
    Core(String),
}

impl From<reqwest::Error> for SdkError {
    fn from(err: reqwest::Error) -> Self {
        SdkError::Http(err.to_string())
    }
}

impl From<serde_json::Error> for SdkError {
    fn from(err: serde_json::Error) -> Self {
        SdkError::Serialization(err.to_string())
    }
}

impl From<url::ParseError> for SdkError {
    fn from(err: url::ParseError) -> Self {
        SdkError::UrlParsing(err.to_string())
    }
}

impl From<ingest_core::Error> for SdkError {
    fn from(err: ingest_core::Error) -> Self {
        SdkError::Core(err.to_string())
    }
}

impl serde::Serialize for SdkError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SdkError", 2)?;
        match self {
            SdkError::Http(msg) => {
                state.serialize_field("type", "Http")?;
                state.serialize_field("message", msg)?;
            }
            SdkError::Serialization(msg) => {
                state.serialize_field("type", "Serialization")?;
                state.serialize_field("message", msg)?;
            }
            SdkError::UrlParsing(msg) => {
                state.serialize_field("type", "UrlParsing")?;
                state.serialize_field("message", msg)?;
            }
            SdkError::Authentication { message } => {
                state.serialize_field("type", "Authentication")?;
                state.serialize_field("message", message)?;
            }
            SdkError::Api { status, message } => {
                state.serialize_field("type", "Api")?;
                state.serialize_field("message", &format!("{status}: {message}"))?;
            }
            SdkError::Configuration { message } => {
                state.serialize_field("type", "Configuration")?;
                state.serialize_field("message", message)?;
            }
            SdkError::Execution { message } => {
                state.serialize_field("type", "Execution")?;
                state.serialize_field("message", message)?;
            }
            SdkError::LocalServer { message } => {
                state.serialize_field("type", "LocalServer")?;
                state.serialize_field("message", message)?;
            }
            SdkError::Validation { field, message } => {
                state.serialize_field("type", "Validation")?;
                state.serialize_field("message", &format!("{field}: {message}"))?;
            }
            SdkError::Generic { message } => {
                state.serialize_field("type", "Generic")?;
                state.serialize_field("message", message)?;
            }
            SdkError::Core(msg) => {
                state.serialize_field("type", "Core")?;
                state.serialize_field("message", msg)?;
            }
        }
        state.end()
    }
}

impl SdkError {
    /// Create an authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create an API error
    pub fn api(status: u16, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create an execution error
    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }

    /// Create a local server error
    pub fn local_server(message: impl Into<String>) -> Self {
        Self::LocalServer {
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Generic {
            message: format!("Timeout: {}", message.into()),
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            SdkError::Http(msg) => msg.contains("timeout") || msg.contains("connect"),
            SdkError::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Get the error category for telemetry
    pub fn category(&self) -> &'static str {
        match self {
            SdkError::Http(_) => "http",
            SdkError::Serialization(_) => "serialization",
            SdkError::UrlParsing(_) => "url_parsing",
            SdkError::Authentication { .. } => "authentication",
            SdkError::Api { .. } => "api",
            SdkError::Configuration { .. } => "configuration",
            SdkError::Execution { .. } => "execution",
            SdkError::LocalServer { .. } => "local_server",
            SdkError::Validation { .. } => "validation",
            SdkError::Generic { .. } => "generic",
            SdkError::Core(_) => "core",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_authentication_error() {
        let fixture = SdkError::authentication("Invalid API key");
        let actual = fixture.to_string();
        let expected = "Authentication error: Invalid API key";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_api_error() {
        let fixture = SdkError::api(404, "Not found");
        let actual = fixture.to_string();
        let expected = "API error: 404 - Not found";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_validation_error() {
        let fixture = SdkError::validation("email", "Invalid format");
        let actual = fixture.to_string();
        let expected = "Validation error: email: Invalid format";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_retryable() {
        let fixture = SdkError::api(500, "Internal server error");
        assert!(fixture.is_retryable());

        let fixture = SdkError::api(404, "Not found");
        assert!(!fixture.is_retryable());
    }

    #[test]
    fn test_error_category() {
        let fixture = SdkError::authentication("test");
        let actual = fixture.category();
        let expected = "authentication";
        assert_eq!(actual, expected);
    }
}
