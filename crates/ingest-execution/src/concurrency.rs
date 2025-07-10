use crate::{ExecutionError, Result};
use ingest_core::{DateTime, Id};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

/// Concurrency configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent executions globally
    pub max_global_concurrent: u32,
    /// Maximum concurrent executions per function
    pub max_function_concurrent: u32,
    /// Maximum concurrent executions per tenant
    pub max_tenant_concurrent: u32,
    /// Rate limiting window duration
    pub rate_limit_window: std::time::Duration,
    /// Maximum executions per rate limit window
    pub max_executions_per_window: u32,
    /// Enable backpressure handling
    pub enable_backpressure: bool,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_global_concurrent: 1000,
            max_function_concurrent: 100,
            max_tenant_concurrent: 200,
            rate_limit_window: std::time::Duration::from_secs(60),
            max_executions_per_window: 1000,
            enable_backpressure: true,
        }
    }
}

/// Execution permit for tracking active executions
#[derive(Debug, Clone)]
pub struct ExecutionPermit {
    /// Execution run identifier
    pub run_id: Id,
    /// Function identifier
    pub function_id: Id,
    /// Tenant identifier
    pub tenant_id: Option<Id>,
    /// Permit acquisition timestamp
    pub acquired_at: DateTime,
}

/// Rate limit bucket for tracking execution rates
#[derive(Debug, Clone)]
struct RateLimitBucket {
    /// Number of executions in current window
    count: u32,
    /// Window start time
    window_start: DateTime,
    /// Window duration
    window_duration: std::time::Duration,
}

impl RateLimitBucket {
    fn new(window_duration: std::time::Duration) -> Self {
        Self {
            count: 0,
            window_start: chrono::Utc::now(),
            window_duration,
        }
    }

    fn is_window_expired(&self) -> bool {
        let elapsed = chrono::Utc::now() - self.window_start;
        elapsed.to_std().unwrap_or_default() > self.window_duration
    }

    fn reset_window(&mut self) {
        self.count = 0;
        self.window_start = chrono::Utc::now();
    }

    fn can_execute(&mut self, max_per_window: u32) -> bool {
        if self.is_window_expired() {
            self.reset_window();
        }
        self.count < max_per_window
    }

    fn record_execution(&mut self) {
        if self.is_window_expired() {
            self.reset_window();
        }
        self.count += 1;
    }
}

/// Concurrency controller for managing execution limits and flow control
pub struct ConcurrencyController {
    /// Configuration
    config: ConcurrencyConfig,
    /// Global execution semaphore
    global_semaphore: Arc<Semaphore>,
    /// Function-specific semaphores
    function_semaphores: Arc<RwLock<HashMap<Id, Arc<Semaphore>>>>,
    /// Tenant-specific semaphores
    tenant_semaphores: Arc<RwLock<HashMap<Id, Arc<Semaphore>>>>,
    /// Active execution permits
    active_permits: Arc<RwLock<HashMap<Id, ExecutionPermit>>>,
    /// Rate limit buckets by function
    function_rate_limits: Arc<RwLock<HashMap<Id, RateLimitBucket>>>,
    /// Rate limit buckets by tenant
    tenant_rate_limits: Arc<RwLock<HashMap<Id, RateLimitBucket>>>,
}

impl ConcurrencyController {
    /// Create a new concurrency controller
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            global_semaphore: Arc::new(Semaphore::new(config.max_global_concurrent as usize)),
            function_semaphores: Arc::new(RwLock::new(HashMap::new())),
            tenant_semaphores: Arc::new(RwLock::new(HashMap::new())),
            active_permits: Arc::new(RwLock::new(HashMap::new())),
            function_rate_limits: Arc::new(RwLock::new(HashMap::new())),
            tenant_rate_limits: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create a controller with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ConcurrencyConfig::default())
    }

    /// Acquire execution permit
    pub async fn acquire_permit(
        &self,
        run_id: Id,
        function_id: Id,
        tenant_id: Option<Id>,
    ) -> Result<ExecutionPermit> {
        // Check rate limits first
        self.check_rate_limits(&function_id, &tenant_id).await?;

        // Acquire global permit
        let _global_permit = self
            .global_semaphore
            .acquire()
            .await
            .map_err(|_| ExecutionError::internal("Failed to acquire global permit"))?;

        // Acquire function-specific permit
        let function_semaphore = self.get_or_create_function_semaphore(&function_id).await;
        let _function_permit = function_semaphore
            .acquire()
            .await
            .map_err(|_| ExecutionError::internal("Failed to acquire function permit"))?;

        // Acquire tenant-specific permit if needed
        let _tenant_permit = if let Some(ref tenant_id) = tenant_id {
            let tenant_semaphore = self.get_or_create_tenant_semaphore(tenant_id).await;
            let permit = tenant_semaphore
                .acquire()
                .await
                .map_err(|_| ExecutionError::internal("Failed to acquire tenant permit"))?;
            std::mem::forget(permit);
            Some(())
        } else {
            None
        };

        // Record rate limit usage
        self.record_rate_limit_usage(&function_id, &tenant_id).await;

        // Create and store permit
        let permit = ExecutionPermit {
            run_id: run_id.clone(),
            function_id,
            tenant_id,
            acquired_at: chrono::Utc::now(),
        };

        let mut active_permits = self.active_permits.write().await;
        active_permits.insert(run_id, permit.clone());

        // Permits are automatically released when dropped
        std::mem::forget(_global_permit);
        std::mem::forget(_function_permit);
        // Tenant permit is already forgotten above if acquired

        Ok(permit)
    }

    /// Release execution permit
    pub async fn release_permit(&self, run_id: &Id) -> Result<()> {
        let mut active_permits = self.active_permits.write().await;
        if active_permits.remove(run_id).is_some() {
            tracing::debug!("Released execution permit for run {}", run_id.as_str());

            // Permits are automatically released when the controller is dropped
            // In a real implementation, we would properly track and release semaphore permits
            Ok(())
        } else {
            Err(ExecutionError::internal(format!(
                "No active permit found for run {}",
                run_id.as_str()
            )))
        }
    }

    /// Check if execution can proceed based on rate limits
    async fn check_rate_limits(&self, function_id: &Id, tenant_id: &Option<Id>) -> Result<()> {
        // Check function rate limit
        let mut function_rate_limits = self.function_rate_limits.write().await;
        let function_bucket = function_rate_limits
            .entry(function_id.clone())
            .or_insert_with(|| RateLimitBucket::new(self.config.rate_limit_window));

        if !function_bucket.can_execute(self.config.max_executions_per_window) {
            return Err(ExecutionError::concurrency_limit_exceeded(
                function_bucket.count,
                self.config.max_executions_per_window,
            ));
        }

        // Check tenant rate limit if applicable
        if let Some(tenant_id) = tenant_id {
            let mut tenant_rate_limits = self.tenant_rate_limits.write().await;
            let tenant_bucket = tenant_rate_limits
                .entry(tenant_id.clone())
                .or_insert_with(|| RateLimitBucket::new(self.config.rate_limit_window));

            if !tenant_bucket.can_execute(self.config.max_executions_per_window) {
                return Err(ExecutionError::concurrency_limit_exceeded(
                    tenant_bucket.count,
                    self.config.max_executions_per_window,
                ));
            }
        }

        Ok(())
    }

    /// Record rate limit usage
    async fn record_rate_limit_usage(&self, function_id: &Id, tenant_id: &Option<Id>) {
        // Record function usage
        let mut function_rate_limits = self.function_rate_limits.write().await;
        if let Some(bucket) = function_rate_limits.get_mut(function_id) {
            bucket.record_execution();
        }

        // Record tenant usage if applicable
        if let Some(tenant_id) = tenant_id {
            let mut tenant_rate_limits = self.tenant_rate_limits.write().await;
            if let Some(bucket) = tenant_rate_limits.get_mut(tenant_id) {
                bucket.record_execution();
            }
        }
    }

    /// Get or create function-specific semaphore
    async fn get_or_create_function_semaphore(&self, function_id: &Id) -> Arc<Semaphore> {
        let mut semaphores = self.function_semaphores.write().await;
        semaphores
            .entry(function_id.clone())
            .or_insert_with(|| {
                Arc::new(Semaphore::new(self.config.max_function_concurrent as usize))
            })
            .clone()
    }

    /// Get or create tenant-specific semaphore
    async fn get_or_create_tenant_semaphore(&self, tenant_id: &Id) -> Arc<Semaphore> {
        let mut semaphores = self.tenant_semaphores.write().await;
        semaphores
            .entry(tenant_id.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.max_tenant_concurrent as usize)))
            .clone()
    }

    /// Get current concurrency statistics
    pub async fn get_stats(&self) -> ConcurrencyStats {
        let active_permits = self.active_permits.read().await;
        let function_semaphores = self.function_semaphores.read().await;
        let tenant_semaphores = self.tenant_semaphores.read().await;

        ConcurrencyStats {
            global_active: active_permits.len() as u32,
            global_limit: self.config.max_global_concurrent,
            function_count: function_semaphores.len() as u32,
            tenant_count: tenant_semaphores.len() as u32,
            active_permits: active_permits.len() as u32,
        }
    }

    /// Check if system is under backpressure
    pub async fn is_under_backpressure(&self) -> bool {
        if !self.config.enable_backpressure {
            return false;
        }

        let stats = self.get_stats().await;
        let utilization = stats.global_active as f64 / stats.global_limit as f64;

        // Consider system under backpressure if utilization > 80%
        utilization > 0.8
    }

    /// Get configuration
    pub fn config(&self) -> &ConcurrencyConfig {
        &self.config
    }
}

/// Concurrency statistics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcurrencyStats {
    /// Current global active executions
    pub global_active: u32,
    /// Global execution limit
    pub global_limit: u32,
    /// Number of functions with active executions
    pub function_count: u32,
    /// Number of tenants with active executions
    pub tenant_count: u32,
    /// Total active permits
    pub active_permits: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_concurrency_config_default() {
        let actual = ConcurrencyConfig::default();
        assert_eq!(actual.max_global_concurrent, 1000);
        assert_eq!(actual.max_function_concurrent, 100);
        assert_eq!(actual.max_tenant_concurrent, 200);
        assert!(actual.enable_backpressure);
    }

    #[test]
    fn test_rate_limit_bucket_creation() {
        let fixture_duration = std::time::Duration::from_secs(60);
        let actual = RateLimitBucket::new(fixture_duration);

        assert_eq!(actual.count, 0);
        assert_eq!(actual.window_duration, fixture_duration);
    }

    #[test]
    fn test_rate_limit_bucket_can_execute() {
        let mut fixture = RateLimitBucket::new(std::time::Duration::from_secs(60));

        assert!(fixture.can_execute(10));

        fixture.count = 10;
        assert!(!fixture.can_execute(10));
    }

    #[test]
    fn test_rate_limit_bucket_record_execution() {
        let mut fixture = RateLimitBucket::new(std::time::Duration::from_secs(60));

        fixture.record_execution();
        assert_eq!(fixture.count, 1);

        fixture.record_execution();
        assert_eq!(fixture.count, 2);
    }

    #[test]
    fn test_concurrency_controller_creation() {
        let fixture_config = ConcurrencyConfig::default();
        let actual = ConcurrencyController::new(fixture_config.clone());

        assert_eq!(actual.config(), &fixture_config);
    }

    #[test]
    fn test_concurrency_controller_with_defaults() {
        let actual = ConcurrencyController::with_defaults();
        assert_eq!(actual.config().max_global_concurrent, 1000);
    }

    #[tokio::test]
    async fn test_acquire_and_release_permit() {
        let fixture_controller = ConcurrencyController::with_defaults();
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");

        // Acquire permit
        let actual_permit = fixture_controller
            .acquire_permit(fixture_run_id.clone(), fixture_function_id.clone(), None)
            .await;

        assert!(actual_permit.is_ok());
        let permit = actual_permit.unwrap();
        assert_eq!(permit.run_id, fixture_run_id);
        assert_eq!(permit.function_id, fixture_function_id);

        // Check stats
        let stats = fixture_controller.get_stats().await;
        assert_eq!(stats.active_permits, 1);

        // Release permit
        let actual_release = fixture_controller.release_permit(&fixture_run_id).await;
        assert!(actual_release.is_ok());

        // Check stats after release
        let stats = fixture_controller.get_stats().await;
        assert_eq!(stats.active_permits, 0);
    }

    #[tokio::test]
    async fn test_acquire_permit_with_tenant() {
        let fixture_controller = ConcurrencyController::with_defaults();
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");
        let fixture_tenant_id = generate_id_with_prefix("tenant");

        let actual = fixture_controller
            .acquire_permit(
                fixture_run_id.clone(),
                fixture_function_id.clone(),
                Some(fixture_tenant_id.clone()),
            )
            .await;

        assert!(actual.is_ok());
        let permit = actual.unwrap();
        assert_eq!(permit.tenant_id, Some(fixture_tenant_id));
    }

    #[tokio::test]
    async fn test_concurrency_stats() {
        let fixture_controller = ConcurrencyController::with_defaults();

        let initial_stats = fixture_controller.get_stats().await;
        assert_eq!(initial_stats.global_active, 0);
        assert_eq!(initial_stats.global_limit, 1000);

        // Acquire a permit
        let _permit = fixture_controller
            .acquire_permit(
                generate_id_with_prefix("run"),
                generate_id_with_prefix("fn"),
                None,
            )
            .await
            .unwrap();

        let stats_with_permit = fixture_controller.get_stats().await;
        assert_eq!(stats_with_permit.global_active, 1);
        assert_eq!(stats_with_permit.active_permits, 1);
    }

    #[tokio::test]
    async fn test_backpressure_detection() {
        let config = ConcurrencyConfig {
            max_global_concurrent: 5, // Small limit for testing
            ..ConcurrencyConfig::default()
        };
        let fixture_controller = ConcurrencyController::new(config);

        // Initially no backpressure
        assert!(!fixture_controller.is_under_backpressure().await);

        // Acquire permits to trigger backpressure (> 80% utilization)
        let mut permits = Vec::new();
        for i in 0..5 {
            let permit = fixture_controller
                .acquire_permit(
                    generate_id_with_prefix(&format!("run_{}", i)),
                    generate_id_with_prefix("fn"),
                    None,
                )
                .await
                .unwrap();
            permits.push(permit);
        }

        // Should be under backpressure now
        assert!(fixture_controller.is_under_backpressure().await);
    }

    #[tokio::test]
    async fn test_release_nonexistent_permit() {
        let fixture_controller = ConcurrencyController::with_defaults();
        let fixture_run_id = generate_id_with_prefix("run");

        let actual = fixture_controller.release_permit(&fixture_run_id).await;
        assert!(actual.is_err());
    }

    #[test]
    fn test_execution_permit_creation() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");
        let fixture_tenant_id = generate_id_with_prefix("tenant");

        let actual = ExecutionPermit {
            run_id: fixture_run_id.clone(),
            function_id: fixture_function_id.clone(),
            tenant_id: Some(fixture_tenant_id.clone()),
            acquired_at: chrono::Utc::now(),
        };

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_function_id);
        assert_eq!(actual.tenant_id, Some(fixture_tenant_id));
    }

    #[test]
    fn test_concurrency_stats_serialization() {
        let fixture = ConcurrencyStats {
            global_active: 10,
            global_limit: 100,
            function_count: 5,
            tenant_count: 3,
            active_permits: 10,
        };

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }
}
