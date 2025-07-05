use thiserror::Error;

/// Configuration error types
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration loading error: {message}")]
    Loading { message: String },

    #[error("Configuration validation error: {message}")]
    Validation { message: String },

    #[error("Environment variable error: {message}")]
    Environment { message: String },

    #[error("File error: {message}")]
    File { message: String },

    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: toml::de::Error,
    },

    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Config builder error: {source}")]
    ConfigBuilder {
        #[from]
        source: config::ConfigError,
    },
}

impl ConfigError {
    /// Create a loading error
    pub fn loading(message: impl Into<String>) -> Self {
        Self::Loading {
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create an environment error
    pub fn environment(message: impl Into<String>) -> Self {
        Self::Environment {
            message: message.into(),
        }
    }

    /// Create a file error
    pub fn file(message: impl Into<String>) -> Self {
        Self::File {
            message: message.into(),
        }
    }
}

/// Result type alias for configuration operations
pub type Result<T> = std::result::Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_config_error_loading() {
        let fixture = "Failed to load config";
        let actual = ConfigError::loading(fixture);
        assert!(matches!(actual, ConfigError::Loading { .. }));
        assert_eq!(
            format!("{}", actual),
            "Configuration loading error: Failed to load config"
        );
    }

    #[test]
    fn test_config_error_validation() {
        let fixture = "Invalid configuration";
        let actual = ConfigError::validation(fixture);
        assert!(matches!(actual, ConfigError::Validation { .. }));
        assert_eq!(
            format!("{}", actual),
            "Configuration validation error: Invalid configuration"
        );
    }

    #[test]
    fn test_config_error_environment() {
        let fixture = "Missing environment variable";
        let actual = ConfigError::environment(fixture);
        assert!(matches!(actual, ConfigError::Environment { .. }));
        assert_eq!(
            format!("{}", actual),
            "Environment variable error: Missing environment variable"
        );
    }

    #[test]
    fn test_config_error_file() {
        let fixture = "File not found";
        let actual = ConfigError::file(fixture);
        assert!(matches!(actual, ConfigError::File { .. }));
        assert_eq!(format!("{}", actual), "File error: File not found");
    }
}
