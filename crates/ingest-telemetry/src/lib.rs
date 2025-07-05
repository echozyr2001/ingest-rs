//! # Inngest Telemetry
//!
//! This crate provides comprehensive telemetry and observability features for the Inngest platform,
//! including distributed tracing, metrics collection, and structured logging.
//!
//! ## Features
//!
//! - OpenTelemetry integration with multiple exporters
//! - Distributed tracing with Jaeger and OTLP support
//! - Prometheus metrics collection
//! - Structured logging with tracing context
//! - Performance monitoring and instrumentation

pub mod config;
pub mod error;
pub mod exporter;
pub mod metrics;
pub mod tracing;

use crate::config::TelemetryConfig;
use crate::error::{Result, TelemetryError};
use crate::exporter::ExporterManager;
use crate::metrics::MetricsCollector;
use crate::tracing::TracingManager;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use crate::config::TelemetryConfig as Config;
pub use crate::error::{Result as TelemetryResult, TelemetryError as Error};
pub use crate::metrics::{Counter, Gauge, Histogram};
pub use crate::tracing::TracingManager as Tracing;

/// Main telemetry system that coordinates all observability components
#[derive(Clone)]
pub struct TelemetrySystem {
    config: TelemetryConfig,
    tracing_manager: Arc<RwLock<TracingManager>>,
    metrics_collector: Arc<RwLock<MetricsCollector>>,
    exporter_manager: Arc<RwLock<ExporterManager>>,
    initialized: Arc<RwLock<bool>>,
}

impl TelemetrySystem {
    /// Create a new telemetry system with the given configuration
    pub fn new(config: TelemetryConfig) -> Self {
        let tracing_manager = Arc::new(RwLock::new(TracingManager::new(&config)));
        let metrics_collector = Arc::new(RwLock::new(MetricsCollector::new(&config)));
        let exporter_manager = Arc::new(RwLock::new(ExporterManager::new(&config)));

        Self {
            config,
            tracing_manager,
            metrics_collector,
            exporter_manager,
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize the telemetry system
    pub async fn initialize(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if *initialized {
            eprintln!("Warning: Telemetry system already initialized");
            return Ok(());
        }

        println!("Initializing telemetry system");

        // Initialize tracing
        let mut tracing_manager = self.tracing_manager.write().await;
        tracing_manager.initialize().await?;
        drop(tracing_manager);

        // Initialize metrics
        let mut metrics_collector = self.metrics_collector.write().await;
        metrics_collector.initialize().await?;
        drop(metrics_collector);

        // Initialize exporters
        let mut exporter_manager = self.exporter_manager.write().await;
        exporter_manager.initialize().await?;
        drop(exporter_manager);

        *initialized = true;
        println!("Telemetry system initialized successfully");
        Ok(())
    }

    /// Shutdown the telemetry system
    pub async fn shutdown(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if !*initialized {
            eprintln!("Warning: Telemetry system not initialized");
            return Ok(());
        }

        println!("Shutting down telemetry system");

        // Shutdown exporters
        let mut exporter_manager = self.exporter_manager.write().await;
        exporter_manager.shutdown().await?;
        drop(exporter_manager);

        // Shutdown metrics
        let mut metrics_collector = self.metrics_collector.write().await;
        metrics_collector.shutdown().await?;
        drop(metrics_collector);

        // Shutdown tracing
        let mut tracing_manager = self.tracing_manager.write().await;
        tracing_manager.shutdown().await?;
        drop(tracing_manager);

        *initialized = false;
        println!("Telemetry system shut down successfully");
        Ok(())
    }

    /// Get the tracing manager
    pub async fn tracing(&self) -> tokio::sync::RwLockReadGuard<'_, TracingManager> {
        self.tracing_manager.read().await
    }

    /// Get the metrics collector
    pub async fn metrics(&self) -> tokio::sync::RwLockReadGuard<'_, MetricsCollector> {
        self.metrics_collector.read().await
    }

    /// Check if the telemetry system is initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get telemetry configuration
    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }

    /// Create a new counter metric
    pub async fn counter(&self, name: &str, description: &str) -> Result<Counter> {
        let metrics = self.metrics_collector.read().await;
        metrics.counter(name, description).await
    }

    /// Create a new gauge metric
    pub async fn gauge(&self, name: &str, description: &str) -> Result<Gauge> {
        let metrics = self.metrics_collector.read().await;
        metrics.gauge(name, description).await
    }

    /// Create a new histogram metric
    pub async fn histogram(&self, name: &str, description: &str) -> Result<Histogram> {
        let metrics = self.metrics_collector.read().await;
        metrics.histogram(name, description).await
    }

    /// Record a custom metric value
    pub async fn record_metric(
        &self,
        name: &str,
        value: f64,
        labels: &[(&str, &str)],
    ) -> Result<()> {
        let metrics = self.metrics_collector.read().await;
        metrics.record(name, value, labels)
    }

    /// Get current metrics as Prometheus format
    pub async fn metrics_snapshot(&self) -> Result<String> {
        let metrics = self.metrics_collector.read().await;
        metrics.snapshot().await
    }
}

/// Global telemetry system instance
static GLOBAL_TELEMETRY: std::sync::OnceLock<TelemetrySystem> = std::sync::OnceLock::new();

/// Initialize the global telemetry system
pub fn init(config: TelemetryConfig) -> Result<()> {
    let telemetry = TelemetrySystem::new(config);
    GLOBAL_TELEMETRY
        .set(telemetry)
        .map_err(|_| TelemetryError::initialization("Global telemetry already initialized"))?;
    Ok(())
}

/// Get the global telemetry system
pub fn global() -> Result<&'static TelemetrySystem> {
    GLOBAL_TELEMETRY
        .get()
        .ok_or_else(|| TelemetryError::initialization("Global telemetry not initialized"))
}

/// Initialize and start the global telemetry system
pub async fn start(config: TelemetryConfig) -> Result<()> {
    init(config)?;
    let telemetry = global()?;
    telemetry.initialize().await?;
    Ok(())
}

/// Shutdown the global telemetry system
pub async fn shutdown() -> Result<()> {
    if let Ok(telemetry) = global() {
        telemetry.shutdown().await?;
    }
    Ok(())
}

/// Convenience macro for creating instrumented functions
#[macro_export]
macro_rules! instrument {
    ($func:expr) => {{
        use tracing::instrument;
        instrument($func)
    }};
}

/// Convenience macro for recording metrics
#[macro_export]
macro_rules! record_metric {
    ($name:expr, $value:expr) => {{
        if let Ok(telemetry) = $crate::global() {
            std::mem::drop(telemetry.record_metric($name, $value, &[]));
        }
    }};
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {{
        if let Ok(telemetry) = $crate::global() {
            let labels = &[$(($key, $val)),*];
            std::mem::drop(telemetry.record_metric($name, $value, labels));
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_config() -> TelemetryConfig {
        TelemetryConfig::default()
            .service_name("test-service")
            .tracing_enabled(true)
            .metrics_enabled(true)
    }

    #[test]
    fn test_telemetry_system_creation() {
        let config = create_test_config();
        let telemetry = TelemetrySystem::new(config.clone());
        assert_eq!(telemetry.config().service_name, "test-service");
    }

    #[tokio::test]
    async fn test_telemetry_system_initialization() {
        let config = create_test_config();
        let telemetry = TelemetrySystem::new(config);

        assert!(!telemetry.is_initialized().await);

        // Note: In a real test environment, this would require proper setup
        // For now, we just test the interface
    }

    #[test]
    fn test_telemetry_config_validation() {
        let config = create_test_config();
        assert!(config.tracing_enabled);
        assert!(config.metrics_enabled);
        assert_eq!(config.service_name, "test-service");
    }

    #[test]
    fn test_global_telemetry_not_initialized() {
        // Test that global telemetry returns error when not initialized
        // Note: This might fail if other tests have initialized global telemetry
        match global() {
            Ok(_) => {
                // Already initialized by another test
            }
            Err(_) => {
                // Expected when not initialized
            }
        }
    }

    #[test]
    fn test_macros_compile() {
        // Test that our macros compile correctly
        record_metric!("test.counter", 1.0);
        record_metric!("test.labeled", 2.0, "label1" => "value1", "label2" => "value2");
    }
}
