//! # Inngest Storage
//!
//! This crate provides storage abstractions and implementations for the Inngest platform,
//! supporting multiple storage backends including PostgreSQL and Redis.
//!
//! ## Features
//!
//! - Database connection pooling
//! - Transaction management
//! - Multi-backend support
//! - Health checks and monitoring

pub mod error;
pub mod postgres;
pub mod traits;

use ingest_config::DatabaseConfig;

pub use error::{Result, StorageError};
pub use postgres::PostgresStorage;
pub use traits::{Storage, Transaction};

/// Storage factory for creating storage instances
pub struct StorageFactory;

impl StorageFactory {
    /// Create a PostgreSQL storage instance
    pub async fn create_postgres(config: &DatabaseConfig) -> Result<PostgresStorage> {
        PostgresStorage::new(config).await
    }
}

/// Health check result
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HealthCheck {
    /// Whether the storage is healthy
    pub healthy: bool,
    /// Optional error message
    pub message: Option<String>,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HealthCheck {
    /// Create a healthy check result
    pub fn healthy(response_time_ms: u64) -> Self {
        Self {
            healthy: true,
            message: None,
            response_time_ms,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an unhealthy check result
    pub fn unhealthy(message: impl Into<String>, response_time_ms: u64) -> Self {
        Self {
            healthy: false,
            message: Some(message.into()),
            response_time_ms,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_health_check_healthy() {
        let fixture = HealthCheck::healthy(50);
        assert!(fixture.healthy);
        assert!(fixture.message.is_none());
        assert_eq!(fixture.response_time_ms, 50);
    }

    #[test]
    fn test_health_check_unhealthy() {
        let fixture = HealthCheck::unhealthy("Connection failed", 1000);
        assert!(!fixture.healthy);
        assert_eq!(fixture.message, Some("Connection failed".to_string()));
        assert_eq!(fixture.response_time_ms, 1000);
    }

    #[test]
    fn test_health_check_serialization() {
        let fixture = HealthCheck::healthy(25);
        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_health_check_deserialization() {
        let fixture = HealthCheck::healthy(25);
        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: HealthCheck = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.healthy, fixture.healthy);
        assert_eq!(actual.response_time_ms, fixture.response_time_ms);
    }
}
