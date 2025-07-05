use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use derive_setters::Setters;
use ingest_storage::{PostgresStorage, RedisStorage, Storage};
use ingest_telemetry::TelemetrySystem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Health status of a component
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component is degraded but functional
    Degraded,
    /// Component is unhealthy
    Unhealthy,
}

impl HealthStatus {
    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Check if the status is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded)
    }

    /// Check if the status is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy)
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct HealthCheck {
    /// Unique identifier for this check
    pub id: Uuid,
    /// Name of the component being checked
    pub name: String,
    /// Health status
    pub status: HealthStatus,
    /// Optional message with additional details
    pub message: Option<String>,
    /// Timestamp when the check was performed
    pub timestamp: DateTime<Utc>,
    /// Duration of the check
    pub duration: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl HealthCheck {
    /// Create a new health check result
    pub fn new(name: impl Into<String>, status: HealthStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            status,
            message: None,
            timestamp: Utc::now(),
            duration: Duration::from_millis(0),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Trait for components that can be health checked
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform a health check
    async fn check_health(&self) -> HealthCheck;

    /// Get the name of this health checker
    fn name(&self) -> &str;
}

/// Storage health checker
pub struct StorageHealthChecker {
    name: String,
    storage: Arc<PostgresStorage>,
}

impl StorageHealthChecker {
    /// Create a new storage health checker
    pub fn new(name: impl Into<String>, storage: Arc<PostgresStorage>) -> Self {
        Self {
            name: name.into(),
            storage,
        }
    }
}

#[async_trait]
impl HealthChecker for StorageHealthChecker {
    async fn check_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();

        // Try to perform a simple health check operation
        let status = match self.storage.health_check().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        };

        let duration = start.elapsed();

        HealthCheck::new(&self.name, status)
            .duration(duration)
            .with_metadata("type", "storage")
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Redis health checker
pub struct RedisHealthChecker {
    name: String,
    redis: Arc<RedisStorage>,
}

impl RedisHealthChecker {
    /// Create a new Redis health checker
    pub fn new(name: impl Into<String>, redis: Arc<RedisStorage>) -> Self {
        Self {
            name: name.into(),
            redis,
        }
    }
}

#[async_trait]
impl HealthChecker for RedisHealthChecker {
    async fn check_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();

        // Try to ping Redis
        let status = match self.redis.ping().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        };

        let duration = start.elapsed();

        HealthCheck::new(&self.name, status)
            .duration(duration)
            .with_metadata("type", "redis")
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Telemetry health checker
pub struct TelemetryHealthChecker {
    name: String,
    telemetry: Arc<TelemetrySystem>,
}

impl TelemetryHealthChecker {
    /// Create a new telemetry health checker
    pub fn new(name: impl Into<String>, telemetry: Arc<TelemetrySystem>) -> Self {
        Self {
            name: name.into(),
            telemetry,
        }
    }
}

#[async_trait]
impl HealthChecker for TelemetryHealthChecker {
    async fn check_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();

        // Check if telemetry system is initialized
        let status = if self.telemetry.is_initialized().await {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        };

        let duration = start.elapsed();

        HealthCheck::new(&self.name, status)
            .duration(duration)
            .with_metadata("type", "telemetry")
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Health monitor that coordinates all health checks
pub struct HealthMonitor {
    checkers: Arc<RwLock<Vec<Arc<dyn HealthChecker>>>>,
    last_results: Arc<RwLock<Vec<HealthCheck>>>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new() -> Self {
        Self {
            checkers: Arc::new(RwLock::new(Vec::new())),
            last_results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a health checker
    pub async fn register_checker(&self, checker: Arc<dyn HealthChecker>) -> Result<()> {
        let mut checkers = self.checkers.write().await;
        checkers.push(checker);
        Ok(())
    }

    /// Register a storage health checker
    pub async fn register_storage_checker(&self, storage: Arc<PostgresStorage>) -> Result<()> {
        let checker = Arc::new(StorageHealthChecker::new("storage", storage));
        self.register_checker(checker).await
    }

    /// Register a Redis health checker
    pub async fn register_redis_checker(&self, redis: Arc<RedisStorage>) -> Result<()> {
        let checker = Arc::new(RedisHealthChecker::new("redis", redis));
        self.register_checker(checker).await
    }

    /// Register a telemetry health checker
    pub async fn register_telemetry_checker(&self, telemetry: Arc<TelemetrySystem>) -> Result<()> {
        let checker = Arc::new(TelemetryHealthChecker::new("telemetry", telemetry));
        self.register_checker(checker).await
    }

    /// Run all health checks
    pub async fn run_checks(&self) -> Result<Vec<HealthCheck>> {
        let checkers = self.checkers.read().await;
        let mut results = Vec::new();

        for checker in checkers.iter() {
            let result = checker.check_health().await;
            results.push(result);
        }

        // Store the results
        let mut last_results = self.last_results.write().await;
        *last_results = results.clone();

        Ok(results)
    }

    /// Get the overall health status
    pub async fn overall_health(&self) -> HealthStatus {
        let results = self.run_checks().await.unwrap_or_default();

        if results.is_empty() {
            return HealthStatus::Healthy;
        }

        let unhealthy_count = results.iter().filter(|r| r.status.is_unhealthy()).count();
        let degraded_count = results.iter().filter(|r| r.status.is_degraded()).count();

        if unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Get detailed health check results
    pub async fn health_details(&self) -> Vec<HealthCheck> {
        let last_results = self.last_results.read().await;
        last_results.clone()
    }

    /// Get the number of registered checkers
    pub async fn checker_count(&self) -> usize {
        let checkers = self.checkers.read().await;
        checkers.len()
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Healthy.is_degraded());
        assert!(!HealthStatus::Healthy.is_unhealthy());

        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(HealthStatus::Degraded.is_degraded());
        assert!(!HealthStatus::Degraded.is_unhealthy());

        assert!(!HealthStatus::Unhealthy.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_degraded());
        assert!(HealthStatus::Unhealthy.is_unhealthy());
    }

    #[test]
    fn test_health_check_creation() {
        let check = HealthCheck::new("test", HealthStatus::Healthy)
            .message("All good")
            .duration(Duration::from_millis(100))
            .with_metadata("version", "1.0");

        assert_eq!(check.name, "test");
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.message, Some("All good".to_string()));
        assert_eq!(check.duration, Duration::from_millis(100));
        assert_eq!(check.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[tokio::test]
    async fn test_health_monitor() {
        let monitor = HealthMonitor::new();
        assert_eq!(monitor.checker_count().await, 0);

        let status = monitor.overall_health().await;
        assert_eq!(status, HealthStatus::Healthy);

        let details = monitor.health_details().await;
        assert!(details.is_empty());
    }

    #[tokio::test]
    async fn test_health_monitor_overall_status() {
        let monitor = HealthMonitor::new();

        // Test with no checkers
        let status = monitor.overall_health().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_check_setters() {
        let actual = HealthCheck::new("test-service", HealthStatus::Healthy)
            .name("updated-service")
            .status(HealthStatus::Degraded)
            .message("Service is degraded")
            .duration(Duration::from_millis(100))
            .metadata(HashMap::from([("version".to_string(), "1.0".to_string())]));

        assert_eq!(actual.name, "updated-service");
        assert_eq!(actual.status, HealthStatus::Degraded);
        assert_eq!(actual.message, Some("Service is degraded".to_string()));
        assert_eq!(actual.duration, Duration::from_millis(100));
        assert_eq!(actual.metadata.get("version"), Some(&"1.0".to_string()));
    }
}
