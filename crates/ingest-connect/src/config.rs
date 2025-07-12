//! Configuration for the connect system

use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the connect server
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ConnectConfig {
    /// Server bind address
    pub bind_address: String,

    /// Server port
    pub port: u16,

    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// Connection timeout duration
    pub connection_timeout: Duration,

    /// Heartbeat interval
    pub heartbeat_interval: Duration,

    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Connection pool configuration
    pub pool: PoolConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,

    /// Protocol configuration
    pub protocol: ProtocolConfig,

    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

impl Default for ConnectConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 10000,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
            max_message_size: 1024 * 1024, // 1MB
            pool: PoolConfig::default(),
            auth: AuthConfig::default(),
            rate_limit: RateLimitConfig::default(),
            protocol: ProtocolConfig::default(),
            tls: None,
        }
    }
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct PoolConfig {
    /// Maximum connections per pool
    pub max_pool_size: usize,

    /// Minimum connections per pool
    pub min_pool_size: usize,

    /// Connection idle timeout
    pub idle_timeout: Duration,

    /// Pool cleanup interval
    pub cleanup_interval: Duration,

    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_pool_size: 1000,
            min_pool_size: 10,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            cleanup_interval: Duration::from_secs(60), // 1 minute
            load_balancing: LoadBalancingStrategy::RoundRobin,
        }
    }
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,

    /// Least connections
    LeastConnections,

    /// Random selection
    Random,

    /// Weighted round-robin
    WeightedRoundRobin,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct AuthConfig {
    /// Whether authentication is required
    pub required: bool,

    /// JWT secret for token validation
    pub jwt_secret: String,

    /// Token expiration duration
    pub token_expiration: Duration,

    /// Valid API keys
    pub api_keys: Vec<String>,

    /// SDK authentication configuration
    pub sdk: SdkAuthConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            required: true,
            jwt_secret: "default-secret".to_string(),
            token_expiration: Duration::from_secs(3600), // 1 hour
            api_keys: vec![],
            sdk: SdkAuthConfig::default(),
        }
    }
}

/// SDK-specific authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct SdkAuthConfig {
    /// Whether to validate SDK signatures
    pub validate_signatures: bool,

    /// Allowed SDK versions (empty means all)
    pub allowed_versions: Vec<String>,

    /// SDK-specific secrets
    pub sdk_secrets: std::collections::HashMap<String, String>,
}

impl Default for SdkAuthConfig {
    fn default() -> Self {
        Self {
            validate_signatures: true,
            allowed_versions: vec![],
            sdk_secrets: std::collections::HashMap::new(),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled
    pub enabled: bool,

    /// Requests per second limit
    pub requests_per_second: u32,

    /// Burst capacity
    pub burst_capacity: u32,

    /// Rate limit window duration
    pub window_duration: Duration,

    /// Per-connection rate limits
    pub per_connection: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_capacity: 200,
            window_duration: Duration::from_secs(60),
            per_connection: true,
        }
    }
}

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ProtocolConfig {
    /// Supported protocol versions
    pub supported_versions: Vec<String>,

    /// Default protocol version
    pub default_version: String,

    /// Protocol negotiation timeout
    pub negotiation_timeout: Duration,

    /// Message compression settings
    pub compression: CompressionConfig,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            supported_versions: vec!["1.0".to_string(), "1.1".to_string()],
            default_version: "1.1".to_string(),
            negotiation_timeout: Duration::from_secs(10),
            compression: CompressionConfig::default(),
        }
    }
}

/// Message compression configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct CompressionConfig {
    /// Whether compression is enabled
    pub enabled: bool,

    /// Compression algorithm
    pub algorithm: CompressionAlgorithm,

    /// Minimum message size for compression
    pub min_size: usize,

    /// Compression level (1-9)
    pub level: u8,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            min_size: 1024, // 1KB
            level: 6,
        }
    }
}

/// Compression algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// Gzip compression
    Gzip,

    /// Deflate compression
    Deflate,

    /// Brotli compression
    Brotli,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_file: String,

    /// Path to private key file
    pub key_file: String,

    /// Path to CA certificate file
    pub ca_file: Option<String>,

    /// Whether to require client certificates
    pub require_client_cert: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config() {
        let config = ConnectConfig::default();

        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_connections, 10000);
        assert!(config.auth.required);
        assert!(config.rate_limit.enabled);
    }

    #[test]
    fn test_config_setters() {
        let config = ConnectConfig::default()
            .bind_address("0.0.0.0")
            .port(9090u16)
            .max_connections(5000usize);

        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.port, 9090u16);
        assert_eq!(config.max_connections, 5000usize);
    }

    #[test]
    fn test_pool_config() {
        let pool_config = PoolConfig::default();

        assert_eq!(pool_config.max_pool_size, 1000);
        assert_eq!(pool_config.min_pool_size, 10);
        assert!(matches!(
            pool_config.load_balancing,
            LoadBalancingStrategy::RoundRobin
        ));
    }

    #[test]
    fn test_auth_config() {
        let auth_config = AuthConfig::default();

        assert!(auth_config.required);
        assert_eq!(auth_config.jwt_secret, "default-secret");
        assert!(auth_config.sdk.validate_signatures);
    }

    #[test]
    fn test_rate_limit_config() {
        let rate_limit_config = RateLimitConfig::default();

        assert!(rate_limit_config.enabled);
        assert_eq!(rate_limit_config.requests_per_second, 100);
        assert_eq!(rate_limit_config.burst_capacity, 200);
    }

    #[test]
    fn test_protocol_config() {
        let protocol_config = ProtocolConfig::default();

        assert_eq!(protocol_config.supported_versions.len(), 2);
        assert_eq!(protocol_config.default_version, "1.1");
        assert!(protocol_config.compression.enabled);
    }

    #[test]
    fn test_compression_config() {
        let compression_config = CompressionConfig::default();

        assert!(compression_config.enabled);
        assert!(matches!(
            compression_config.algorithm,
            CompressionAlgorithm::Gzip
        ));
        assert_eq!(compression_config.min_size, 1024);
        assert_eq!(compression_config.level, 6);
    }
}
