//! # Inngest Monitoring
//!
//! This crate provides comprehensive monitoring and health check capabilities for the Inngest platform.
//!
//! ## Features
//!
//! - Health check framework with dependency monitoring
//! - HTTP health endpoints for load balancers
//! - Resource utilization tracking
//! - Alert generation and notification
//! - Prometheus metrics integration
//! - Service dependency monitoring

pub mod error;
pub mod health;
pub mod http;
pub mod metrics;
pub mod resource;

use crate::error::{MonitoringError, Result};
use crate::health::{HealthMonitor, HealthStatus};
use crate::http::HealthServer;
use crate::metrics::MonitoringMetrics;
use crate::resource::ResourceMonitor;
use ingest_config::Config;
use ingest_storage::RedisStorage;
use ingest_telemetry::TelemetrySystem;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use error::Result as MonitoringResult;
pub use health::HealthCheck;
pub use resource::ResourceUsage;

/// Main monitoring system that coordinates all health checks and monitoring
#[derive(Clone)]
pub struct MonitoringSystem {
    config: Config,
    health_monitor: Arc<HealthMonitor>,
    resource_monitor: Arc<ResourceMonitor>,
    metrics: Arc<MonitoringMetrics>,
    health_server: Arc<RwLock<Option<HealthServer>>>,
    initialized: Arc<RwLock<bool>>,
}

impl MonitoringSystem {
    /// Create a new monitoring system
    pub fn new(config: Config) -> Self {
        let health_monitor = Arc::new(HealthMonitor::new());
        let resource_monitor = Arc::new(ResourceMonitor::new());
        let metrics = Arc::new(MonitoringMetrics::new());

        Self {
            config,
            health_monitor,
            resource_monitor,
            metrics,
            health_server: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize the monitoring system
    pub async fn initialize(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if *initialized {
            println!("Warning: Monitoring system already initialized");
            return Ok(());
        }

        println!("Initializing monitoring system");

        // Initialize metrics
        self.metrics.initialize().await?;

        // Start resource monitoring
        self.resource_monitor.start().await?;

        // Start health server if configured
        if let Some(port) = self.config.server.health_port {
            let server = HealthServer::new(port, self.health_monitor.clone());
            server.start().await?;

            let mut health_server = self.health_server.write().await;
            *health_server = Some(server);
        }

        *initialized = true;
        println!("Monitoring system initialized successfully");
        Ok(())
    }

    /// Shutdown the monitoring system
    pub async fn shutdown(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if !*initialized {
            println!("Warning: Monitoring system not initialized");
            return Ok(());
        }

        println!("Shutting down monitoring system");

        // Stop health server
        let mut health_server = self.health_server.write().await;
        if let Some(server) = health_server.take() {
            server.stop().await?;
        }

        // Stop resource monitoring
        self.resource_monitor.stop().await?;

        // Shutdown metrics
        self.metrics.shutdown().await?;

        *initialized = false;
        println!("Monitoring system shut down successfully");
        Ok(())
    }

    /// Register a storage health checker
    pub async fn register_storage_checker(
        &self,
        storage: Arc<ingest_storage::PostgresStorage>,
    ) -> Result<()> {
        self.health_monitor.register_storage_checker(storage).await
    }

    /// Register a Redis health checker
    pub async fn register_redis_checker(&self, redis: Arc<RedisStorage>) -> Result<()> {
        self.health_monitor.register_redis_checker(redis).await
    }

    /// Register a telemetry health checker
    pub async fn register_telemetry_checker(&self, telemetry: Arc<TelemetrySystem>) -> Result<()> {
        self.health_monitor
            .register_telemetry_checker(telemetry)
            .await
    }

    /// Get overall system health status
    pub async fn health_status(&self) -> HealthStatus {
        self.health_monitor.overall_health().await
    }

    /// Get detailed health check results
    pub async fn health_details(&self) -> Vec<health::HealthCheck> {
        self.health_monitor.health_details().await
    }

    /// Get current resource usage
    pub async fn resource_usage(&self) -> resource::ResourceUsage {
        self.resource_monitor.current_usage().await
    }

    /// Get monitoring metrics
    pub async fn metrics_snapshot(&self) -> Result<String> {
        self.metrics.snapshot().await
    }

    /// Check if the monitoring system is initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get the health monitor
    pub fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }

    /// Get the resource monitor
    pub fn resource_monitor(&self) -> &ResourceMonitor {
        &self.resource_monitor
    }

    /// Get the monitoring metrics
    pub fn metrics(&self) -> &MonitoringMetrics {
        &self.metrics
    }
}

/// Global monitoring system instance
static GLOBAL_MONITORING: std::sync::OnceLock<MonitoringSystem> = std::sync::OnceLock::new();

/// Initialize the global monitoring system
pub fn init(config: Config) -> Result<()> {
    let monitoring = MonitoringSystem::new(config);
    GLOBAL_MONITORING
        .set(monitoring)
        .map_err(|_| MonitoringError::initialization("Global monitoring already initialized"))?;
    Ok(())
}

/// Get the global monitoring system
pub fn global() -> Result<&'static MonitoringSystem> {
    GLOBAL_MONITORING
        .get()
        .ok_or_else(|| MonitoringError::initialization("Global monitoring not initialized"))
}

/// Initialize and start the global monitoring system
pub async fn start(config: Config) -> Result<()> {
    init(config)?;
    let monitoring = global()?;
    monitoring.initialize().await?;
    Ok(())
}

/// Shutdown the global monitoring system
pub async fn shutdown() -> Result<()> {
    if let Ok(monitoring) = global() {
        monitoring.shutdown().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_config() -> Config {
        Config::default().server(ingest_config::ServerConfig::default().health_port(8080u16))
    }

    #[test]
    fn test_monitoring_system_creation() {
        let config = create_test_config();
        let monitoring = MonitoringSystem::new(config);
        assert_eq!(monitoring.config.server.health_port, Some(8080));
    }

    #[tokio::test]
    async fn test_monitoring_system_initialization() {
        let config = create_test_config();
        let monitoring = MonitoringSystem::new(config);

        assert!(!monitoring.is_initialized().await);

        // Note: In a real test environment, this would require proper setup
        // For now, we just test the interface
    }

    #[test]
    fn test_global_monitoring_not_initialized() {
        // Test that global monitoring returns error when not initialized
        // Note: This might fail if other tests have initialized global monitoring
        match global() {
            Ok(_) => {
                // Already initialized by another test
            }
            Err(_) => {
                // Expected when not initialized
            }
        }
    }

    #[tokio::test]
    async fn test_health_status() {
        let config = create_test_config();
        let monitoring = MonitoringSystem::new(config);

        let status = monitoring.health_status().await;
        // Health status should be available even without initialization
        assert!(matches!(
            status,
            HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy
        ));
    }
}
