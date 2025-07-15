//! # Ingest Server
//!
//! Main server binary for the Inngest durable functions platform.
//!
//! This crate provides the main server implementation that orchestrates all
//! platform services including API, Connect, Execution, Queue, PubSub, and
//! supporting infrastructure services.
//!
//! ## Features
//!
//! - **Service Orchestration**: Manages lifecycle of all platform services
//! - **Graceful Shutdown**: Coordinated shutdown with proper resource cleanup
//! - **Health Monitoring**: Real-time health checks for all services
//! - **Configuration Management**: Comprehensive configuration with validation
//! - **Production Ready**: Full observability and monitoring integration
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ingest_server::{config::ServerConfig, server::IngestServer};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ServerConfig::default();
//!     let server = IngestServer::new(config).await?;
//!     server.start().await?;
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod config;
pub mod error;
pub mod server;
pub mod services;
pub mod shutdown;

// Re-export commonly used types
pub use error::{ConfigError, Result, ServerError};
pub use server::{IngestServer, OverallHealth};
pub use services::{HealthStatus, Service};
pub use shutdown::{ShutdownHandle, ShutdownSignal};

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_health_status_healthy() {
        let fixture = HealthStatus::Healthy;
        let actual = fixture.is_healthy();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_health_status_unhealthy() {
        let fixture = HealthStatus::Unhealthy("test error".to_string());
        let actual = fixture.is_unhealthy();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_health_status_degraded() {
        let fixture = HealthStatus::Degraded("performance issue".to_string());
        let actual = fixture.is_degraded();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_shutdown_signal_equality() {
        let fixture = ShutdownSignal::Graceful;
        let actual = fixture == ShutdownSignal::Graceful;
        let expected = true;
        assert_eq!(actual, expected);
    }
}
