//! Configuration for the API server

use ingest_config::Config;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server bind address
    pub bind_address: String,

    /// Server port
    pub port: u16,

    /// Request timeout duration
    pub request_timeout: Duration,

    /// Maximum request body size in bytes
    pub max_request_size: usize,

    /// CORS configuration
    pub cors: CorsConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,

    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    pub allowed_headers: Vec<String>,

    /// Whether to allow credentials
    pub allow_credentials: bool,

    /// Max age for preflight requests
    pub max_age: Duration,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT secret key
    pub jwt_secret: String,

    /// JWT token expiration
    pub token_expiration: Duration,

    /// Whether authentication is required
    pub required: bool,

    /// API key authentication
    pub api_keys: Vec<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled
    pub enabled: bool,

    /// Requests per minute per IP
    pub requests_per_minute: u32,

    /// Burst capacity
    pub burst_capacity: u32,

    /// Rate limit window duration
    pub window_duration: Duration,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: String,

    /// Path to private key file
    pub key_path: String,

    /// Whether to require client certificates
    pub require_client_cert: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 3000,
            request_timeout: Duration::from_secs(30),
            max_request_size: 10 * 1024 * 1024, // 10MB
            cors: CorsConfig::default(),
            auth: AuthConfig::default(),
            rate_limit: RateLimitConfig::default(),
            tls: None,
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "content-type".to_string(),
                "authorization".to_string(),
                "x-api-key".to_string(),
            ],
            allow_credentials: true,
            max_age: Duration::from_secs(3600),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "default-secret-change-in-production".to_string(),
            token_expiration: Duration::from_secs(3600), // 1 hour
            required: false,
            api_keys: vec![],
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: 1000,
            burst_capacity: 100,
            window_duration: Duration::from_secs(60),
        }
    }
}

impl ApiConfig {
    /// Create API configuration from main config
    pub fn from_config(_config: &Config) -> Self {
        // Extract API-specific configuration from main config
        // This would typically read from config.api section
        Self::default()
    }

    /// Get the full server address
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("Port cannot be 0".to_string());
        }

        if self.auth.jwt_secret.is_empty() {
            return Err("JWT secret cannot be empty".to_string());
        }

        if self.auth.jwt_secret == "default-secret-change-in-production" {
            tracing::warn!("Using default JWT secret - change this in production!");
        }

        if self.max_request_size == 0 {
            return Err("Max request size cannot be 0".to_string());
        }

        if self.rate_limit.enabled && self.rate_limit.requests_per_minute == 0 {
            return Err("Rate limit requests per minute cannot be 0 when enabled".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config() {
        let config = ApiConfig::default();

        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(config.max_request_size, 10 * 1024 * 1024);
        assert!(config.cors.allow_credentials);
        assert!(!config.auth.required);
        assert!(config.rate_limit.enabled);
    }

    #[test]
    fn test_server_address() {
        let config = ApiConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            ..Default::default()
        };

        let address = config.server_address();
        assert_eq!(address, "127.0.0.1:8080");
    }

    #[test]
    fn test_config_validation() {
        let mut config = ApiConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid port
        config.port = 0;
        assert!(config.validate().is_err());

        // Reset and test empty JWT secret
        config.port = 3000;
        config.auth.jwt_secret = String::new();
        assert!(config.validate().is_err());

        // Reset and test zero max request size
        config.auth.jwt_secret = "test-secret".to_string();
        config.max_request_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_cors_config_default() {
        let cors = CorsConfig::default();

        assert_eq!(cors.allowed_origins, vec!["*"]);
        assert!(cors.allowed_methods.contains(&"GET".to_string()));
        assert!(cors.allowed_methods.contains(&"POST".to_string()));
        assert!(cors.allow_credentials);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let rate_limit = RateLimitConfig::default();

        assert!(rate_limit.enabled);
        assert_eq!(rate_limit.requests_per_minute, 1000);
        assert_eq!(rate_limit.burst_capacity, 100);
        assert_eq!(rate_limit.window_duration, Duration::from_secs(60));
    }
}
