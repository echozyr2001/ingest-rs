use derive_setters::Setters;
use serde::{Deserialize, Serialize};

/// Server configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ServerConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Health check port (optional)
    pub health_port: Option<u16>,
    /// Enable graceful shutdown
    pub graceful_shutdown: bool,
    /// Graceful shutdown timeout
    pub shutdown_timeout: Option<std::time::Duration>,
    /// Request timeout
    pub request_timeout: Option<std::time::Duration>,
    /// Keep alive timeout
    pub keep_alive_timeout: Option<std::time::Duration>,
    /// Maximum request body size
    pub max_request_size: Option<u64>,
    /// Number of worker threads
    pub workers: Option<usize>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            health_port: None,
            graceful_shutdown: true,
            shutdown_timeout: Some(std::time::Duration::from_secs(30)),
            request_timeout: Some(std::time::Duration::from_secs(60)),
            keep_alive_timeout: Some(std::time::Duration::from_secs(5)),
            max_request_size: Some(16 * 1024 * 1024), // 16MB
            workers: None,                            // Use system default
        }
    }
}

impl ServerConfig {
    /// Get the server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Validate the server configuration
    pub fn validate(&self) -> crate::Result<()> {
        if self.host.is_empty() {
            return Err(crate::ConfigError::validation(
                "Server host cannot be empty",
            ));
        }

        if self.port == 0 {
            return Err(crate::ConfigError::validation("Server port must be > 0"));
        }

        if let Some(workers) = self.workers {
            if workers == 0 {
                return Err(crate::ConfigError::validation("Worker count must be > 0"));
            }
        }

        if let Some(max_size) = self.max_request_size {
            if max_size == 0 {
                return Err(crate::ConfigError::validation(
                    "Max request size must be > 0",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_server_config_default() {
        let actual = ServerConfig::default();
        assert_eq!(actual.host, "0.0.0.0");
        assert_eq!(actual.port, 8080);
        assert!(actual.graceful_shutdown);
    }

    #[test]
    fn test_server_config_bind_address() {
        let fixture = ServerConfig::default();
        let actual = fixture.bind_address();
        let expected = "0.0.0.0:8080";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_server_config_validation_success() {
        let fixture = ServerConfig::default();
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }
}
