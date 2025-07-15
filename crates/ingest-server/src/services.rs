use crate::error::{Result, ServerError};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{error, info, warn};

/// Health status for services
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
    Unknown,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded(_))
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy(_))
    }
}

/// Service trait for all platform services
#[async_trait]
pub trait Service: Send + Sync {
    /// Start the service
    async fn start(&self) -> Result<()>;

    /// Stop the service gracefully
    async fn stop(&self) -> Result<()>;

    /// Force stop the service (for emergency shutdown)
    async fn force_stop(&self) -> Result<()> {
        self.stop().await
    }

    /// Check service health
    async fn health_check(&self) -> HealthStatus;

    /// Get service name
    fn name(&self) -> &str;

    /// Get service dependencies (services that must start before this one)
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    /// Check if service is ready to start
    async fn is_ready(&self) -> bool {
        true
    }
}

/// Service handle for managing service lifecycle
pub struct ServiceHandle {
    service: Box<dyn Service>,
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ServiceHandle {
    pub fn new(service: Box<dyn Service>) -> Self {
        Self {
            service,
            started: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_started() {
            return Ok(());
        }

        info!("Starting service: {}", self.service.name());

        // Wait for service to be ready
        self.wait_for_ready().await?;

        // Start the service
        self.service.start().await?;

        // Mark as started
        self.started
            .store(true, std::sync::atomic::Ordering::SeqCst);

        info!("Service started successfully: {}", self.service.name());
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if !self.is_started() {
            return Ok(());
        }

        info!("Stopping service: {}", self.service.name());

        self.service.stop().await?;

        // Mark as stopped
        self.started
            .store(false, std::sync::atomic::Ordering::SeqCst);

        info!("Service stopped successfully: {}", self.service.name());
        Ok(())
    }

    pub async fn force_stop(&self) -> Result<()> {
        if !self.is_started() {
            return Ok(());
        }

        warn!("Force stopping service: {}", self.service.name());

        self.service.force_stop().await?;

        // Mark as stopped
        self.started
            .store(false, std::sync::atomic::Ordering::SeqCst);

        warn!("Service force stopped: {}", self.service.name());
        Ok(())
    }

    pub async fn health_check(&self) -> HealthStatus {
        if !self.is_started() {
            return HealthStatus::Unhealthy("Service not started".to_string());
        }

        self.service.health_check().await
    }

    pub fn name(&self) -> &str {
        self.service.name()
    }

    pub fn dependencies(&self) -> Vec<String> {
        self.service.dependencies()
    }

    pub fn is_started(&self) -> bool {
        self.started.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn wait_for_ready(&self) -> Result<()> {
        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.service.is_ready().await {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(ServerError::Service {
            service: self.service.name().to_string(),
            error: "Service not ready within timeout".to_string(),
        })
    }
}

/// Service orchestrator for managing multiple services
pub struct ServiceOrchestrator {
    services: Vec<ServiceHandle>,
    startup_timeout: Duration,
    health_check_interval: Duration,
}

impl ServiceOrchestrator {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
            startup_timeout: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(30),
        }
    }

    pub fn with_startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_timeout = timeout;
        self
    }

    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    pub fn register_service(&mut self, service: Box<dyn Service>) {
        let handle = ServiceHandle::new(service);
        self.services.push(handle);
    }

    pub async fn start_all(&self) -> Result<()> {
        info!("Starting all services in dependency order");

        // Sort services by dependencies
        let ordered_services = self.resolve_dependencies()?;

        // Start services in order
        for service in &ordered_services {
            service.start().await?;
        }

        // Wait for all services to be healthy
        self.wait_for_all_healthy().await?;

        info!("All services started successfully");
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<()> {
        info!("Stopping all services in reverse dependency order");

        // Stop services in reverse dependency order
        let ordered_services = self.resolve_dependencies()?;
        for service in ordered_services.iter().rev() {
            if let Err(e) = service.stop().await {
                error!("Failed to stop service {}: {:?}", service.name(), e);
            }
        }

        info!("All services stopped");
        Ok(())
    }

    pub async fn force_stop_all(&self) -> Result<()> {
        warn!("Force stopping all services");

        // Force stop all services concurrently
        let futures: Vec<_> = self
            .services
            .iter()
            .map(|service| service.force_stop())
            .collect();

        let results = futures::future::join_all(futures).await;

        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                error!(
                    "Failed to force stop service {}: {:?}",
                    self.services[i].name(),
                    e
                );
            }
        }

        warn!("All services force stopped");
        Ok(())
    }

    pub async fn health_check_all(&self) -> Vec<(String, HealthStatus)> {
        let futures: Vec<_> = self
            .services
            .iter()
            .map(|service| async move {
                let name = service.name().to_string();
                let health = service.health_check().await;
                (name, health)
            })
            .collect();

        futures::future::join_all(futures).await
    }

    pub fn get_service(&self, name: &str) -> Option<&ServiceHandle> {
        self.services.iter().find(|s| s.name() == name)
    }

    fn resolve_dependencies(&self) -> Result<Vec<&ServiceHandle>> {
        // Simple topological sort for dependency resolution
        let mut ordered = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp_visited = std::collections::HashSet::new();

        for service in &self.services {
            if !visited.contains(service.name()) {
                self.visit_service(service, &mut ordered, &mut visited, &mut temp_visited)?;
            }
        }

        Ok(ordered)
    }

    fn visit_service<'a>(
        &'a self,
        service: &'a ServiceHandle,
        ordered: &mut Vec<&'a ServiceHandle>,
        visited: &mut std::collections::HashSet<String>,
        temp_visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        let service_name = service.name().to_string();

        if temp_visited.contains(&service_name) {
            return Err(ServerError::Service {
                service: service_name,
                error: "Circular dependency detected".to_string(),
            });
        }

        if visited.contains(&service_name) {
            return Ok(());
        }

        temp_visited.insert(service_name.clone());

        // Visit dependencies first
        for dep_name in service.dependencies() {
            if let Some(dep_service) = self.get_service(&dep_name) {
                self.visit_service(dep_service, ordered, visited, temp_visited)?;
            } else {
                return Err(ServerError::Service {
                    service: service_name,
                    error: format!("Dependency not found: {dep_name}"),
                });
            }
        }

        temp_visited.remove(&service_name);
        visited.insert(service_name);
        ordered.push(service);

        Ok(())
    }

    async fn wait_for_all_healthy(&self) -> Result<()> {
        let timeout = tokio::time::sleep(self.startup_timeout);
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    return Err(ServerError::StartupTimeout);
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let health_checks = self.health_check_all().await;
                    let all_healthy = health_checks.iter().all(|(_, health)| health.is_healthy());

                    if all_healthy {
                        return Ok(());
                    }

                    // Log unhealthy services
                    for (name, health) in &health_checks {
                        if !health.is_healthy() {
                            warn!("Service {} is not healthy: {:?}", name, health);
                        }
                    }
                }
            }
        }
    }
}

impl Default for ServiceOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockService {
        name: String,
        dependencies: Vec<String>,
        started: Arc<AtomicBool>,
        healthy: Arc<AtomicBool>,
    }

    impl MockService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                dependencies: Vec::new(),
                started: Arc::new(AtomicBool::new(false)),
                healthy: Arc::new(AtomicBool::new(true)),
            }
        }

        fn with_dependencies(mut self, deps: Vec<String>) -> Self {
            self.dependencies = deps;
            self
        }
    }

    #[async_trait]
    impl Service for MockService {
        async fn start(&self) -> Result<()> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> Result<()> {
            self.started.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn health_check(&self) -> HealthStatus {
            if self.healthy.load(Ordering::SeqCst) {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy("Mock unhealthy".to_string())
            }
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn dependencies(&self) -> Vec<String> {
            self.dependencies.clone()
        }
    }

    #[tokio::test]
    async fn test_service_handle_lifecycle() {
        let fixture = ServiceHandle::new(Box::new(MockService::new("test")));

        let actual = fixture.is_started();
        let expected = false;
        assert_eq!(actual, expected);

        fixture.start().await.unwrap();

        let actual = fixture.is_started();
        let expected = true;
        assert_eq!(actual, expected);

        fixture.stop().await.unwrap();

        let actual = fixture.is_started();
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_orchestrator_dependency_resolution() {
        let mut fixture = ServiceOrchestrator::new();

        fixture.register_service(Box::new(MockService::new("service_a")));
        fixture.register_service(Box::new(
            MockService::new("service_b").with_dependencies(vec!["service_a".to_string()]),
        ));

        let actual = fixture.resolve_dependencies().unwrap();
        let expected_order = vec!["service_a", "service_b"];
        let actual_order: Vec<&str> = actual.iter().map(|s| s.name()).collect();
        assert_eq!(actual_order, expected_order);
    }

    #[test]
    fn test_health_status() {
        let fixture = HealthStatus::Healthy;
        let actual = fixture.is_healthy();
        let expected = true;
        assert_eq!(actual, expected);

        let fixture = HealthStatus::Unhealthy("error".to_string());
        let actual = fixture.is_unhealthy();
        let expected = true;
        assert_eq!(actual, expected);
    }
}
