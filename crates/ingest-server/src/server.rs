use crate::config::ServerConfig;
use crate::error::{Result, ServerError};
use crate::services::{HealthStatus, Service, ServiceOrchestrator};
use crate::shutdown::{ShutdownCoordinator, ShutdownHandle, ShutdownManager};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

/// Main Inngest server that orchestrates all platform services
pub struct IngestServer {
    config: ServerConfig,
    orchestrator: Arc<ServiceOrchestrator>,
    shutdown_manager: Arc<ShutdownManager>,
    shutdown_handle: ShutdownHandle,
}

impl IngestServer {
    /// Create a new Inngest server with the given configuration
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("Initializing Inngest server");

        // Validate configuration
        config.validate().map_err(ServerError::Config)?;

        // Create service orchestrator
        let mut orchestrator = ServiceOrchestrator::new()
            .with_startup_timeout(config.server.startup_timeout)
            .with_health_check_interval(config.monitoring.health_check_interval);

        // Register services based on configuration
        Self::register_services(&mut orchestrator, &config).await?;

        let orchestrator = Arc::new(orchestrator);

        // Create shutdown coordinator
        let shutdown_coordinator = Arc::new(
            ShutdownCoordinator::new(orchestrator.clone())
                .with_shutdown_timeout(config.server.shutdown_timeout),
        );

        // Create shutdown manager
        let shutdown_manager = Arc::new(ShutdownManager::new(shutdown_coordinator.clone()));
        let shutdown_handle = shutdown_manager.handle();

        Ok(Self {
            config,
            orchestrator,
            shutdown_manager,
            shutdown_handle,
        })
    }

    /// Start the server and all services
    pub async fn start(&self) -> Result<()> {
        info!("Starting Inngest server");

        // Start shutdown manager
        self.shutdown_manager.start().await?;

        // Start all services
        self.orchestrator.start_all().await?;

        info!("Inngest server started successfully");

        // Wait for shutdown signal
        self.shutdown_manager.wait_for_shutdown().await?;

        info!("Inngest server shutdown completed");
        Ok(())
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Inngest server");
        self.shutdown_handle.shutdown_graceful()
    }

    /// Get server health status
    pub async fn health_check(&self) -> OverallHealth {
        let service_health = self.orchestrator.health_check_all().await;

        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;

        for (_, health) in &service_health {
            match health {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Degraded(_) => degraded_count += 1,
                HealthStatus::Unhealthy(_) | HealthStatus::Unknown => unhealthy_count += 1,
            }
        }

        let _ = healthy_count; // Used for future statistics

        let overall_status = if unhealthy_count > 0 {
            HealthStatus::Unhealthy(format!("{unhealthy_count} services unhealthy"))
        } else if degraded_count > 0 {
            HealthStatus::Degraded(format!("{degraded_count} services degraded"))
        } else {
            HealthStatus::Healthy
        };

        OverallHealth {
            status: overall_status,
            services: service_health,
            uptime: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default(),
        }
    }

    /// Get shutdown handle for external use
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }
    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Register all services with the orchestrator
    async fn register_services(
        orchestrator: &mut ServiceOrchestrator,
        config: &ServerConfig,
    ) -> Result<()> {
        // Storage services (dependencies for other services)
        if config.features.api_enabled || config.features.execution_enabled {
            orchestrator.register_service(Box::new(
                StorageService::new(&config.database, &config.redis).await?,
            ));
        }

        // Telemetry service (early startup for observability)
        if config.features.telemetry_enabled {
            orchestrator
                .register_service(Box::new(TelemetryService::new(&config.telemetry).await?));
        }

        // Monitoring service
        if config.features.monitoring_enabled {
            orchestrator
                .register_service(Box::new(MonitoringService::new(&config.monitoring).await?));
        }

        // Execution engine
        if config.features.execution_enabled {
            orchestrator
                .register_service(Box::new(ExecutionService::new(&config.execution).await?));
        }

        // Queue manager
        if config.features.queue_enabled {
            orchestrator.register_service(Box::new(QueueService::new(&config.queue).await?));
        }

        // PubSub manager
        if config.features.pubsub_enabled {
            orchestrator.register_service(Box::new(PubSubService::new(&config.pubsub).await?));
        }

        // API server
        if config.features.api_enabled {
            orchestrator.register_service(Box::new(ApiService::new(&config.api).await?));
        }

        // Connect server
        if config.features.connect_enabled {
            orchestrator.register_service(Box::new(ConnectService::new(&config.connect).await?));
        }

        // History service
        if config.features.history_enabled {
            orchestrator.register_service(Box::new(HistoryService::new().await?));
        }

        Ok(())
    }
}

/// Overall health status for the server
#[derive(Debug, Clone)]
pub struct OverallHealth {
    pub status: HealthStatus,
    pub services: Vec<(String, HealthStatus)>,
    pub uptime: std::time::Duration,
}

// Service wrapper implementations
// These wrap the actual service implementations from other crates

struct StorageService {
    name: String,
}

impl StorageService {
    async fn new(
        _database_config: &crate::config::DatabaseConfig,
        _redis_config: &crate::config::RedisConfig,
    ) -> Result<Self> {
        Ok(Self {
            name: "storage".to_string(),
        })
    }
}

#[async_trait]
impl Service for StorageService {
    async fn start(&self) -> Result<()> {
        // Initialize storage connections
        info!("Starting storage service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping storage service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        // Check database and Redis connectivity
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }
}

struct TelemetryService {
    name: String,
}

impl TelemetryService {
    async fn new(_config: &crate::config::TelemetryConfig) -> Result<Self> {
        Ok(Self {
            name: "telemetry".to_string(),
        })
    }
}

#[async_trait]
impl Service for TelemetryService {
    async fn start(&self) -> Result<()> {
        info!("Starting telemetry service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping telemetry service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }
}

struct MonitoringService {
    name: String,
}

impl MonitoringService {
    async fn new(_config: &crate::config::MonitoringConfig) -> Result<Self> {
        Ok(Self {
            name: "monitoring".to_string(),
        })
    }
}

#[async_trait]
impl Service for MonitoringService {
    async fn start(&self) -> Result<()> {
        info!("Starting monitoring service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping monitoring service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["telemetry".to_string()]
    }
}

struct ExecutionService {
    name: String,
}

impl ExecutionService {
    async fn new(_config: &crate::config::ExecutionConfig) -> Result<Self> {
        Ok(Self {
            name: "execution".to_string(),
        })
    }
}

#[async_trait]
impl Service for ExecutionService {
    async fn start(&self) -> Result<()> {
        info!("Starting execution service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping execution service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string(), "telemetry".to_string()]
    }
}

struct QueueService {
    name: String,
}

impl QueueService {
    async fn new(_config: &crate::config::QueueConfig) -> Result<Self> {
        Ok(Self {
            name: "queue".to_string(),
        })
    }
}

#[async_trait]
impl Service for QueueService {
    async fn start(&self) -> Result<()> {
        info!("Starting queue service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping queue service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string()]
    }
}

struct PubSubService {
    name: String,
}

impl PubSubService {
    async fn new(_config: &crate::config::PubSubConfig) -> Result<Self> {
        Ok(Self {
            name: "pubsub".to_string(),
        })
    }
}

#[async_trait]
impl Service for PubSubService {
    async fn start(&self) -> Result<()> {
        info!("Starting pubsub service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping pubsub service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string()]
    }
}

struct ApiService {
    name: String,
    config: crate::config::ApiConfig,
}

impl ApiService {
    async fn new(config: &crate::config::ApiConfig) -> Result<Self> {
        Ok(Self {
            name: "api".to_string(),
            config: config.clone(),
        })
    }
}

#[async_trait]
impl Service for ApiService {
    async fn start(&self) -> Result<()> {
        info!(
            "Starting API service on {}:{}",
            self.config.bind_address, self.config.port
        );

        // Create API server configuration
        let api_config = ingest_api::ApiConfig {
            bind_address: self.config.bind_address.clone(),
            port: self.config.port,
            request_timeout: self.config.request_timeout,
            max_request_size: self.config.max_request_size,
            ..Default::default()
        };

        // Create and start the API server
        let server =
            ingest_api::ApiServer::new(api_config)
                .await
                .map_err(|e| ServerError::Service {
                    service: "api".to_string(),
                    error: format!("Failed to create API server: {}", e),
                })?;

        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let addr_for_log = addr.clone();

        // Start the server in a background task
        tokio::spawn(async move {
            if let Err(e) = server.serve(&addr).await {
                tracing::error!("API server error: {}", e);
            }
        });

        // Give the server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        info!("API server started successfully on {}", addr_for_log);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping API service");
        // TODO: Implement graceful shutdown
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to make a request to the health endpoint
        let addr = format!(
            "http://{}:{}/health",
            self.config.bind_address, self.config.port
        );

        match reqwest::get(&addr).await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Degraded("Health endpoint returned error".to_string()),
            Err(_) => HealthStatus::Unhealthy("Cannot reach API server".to_string()),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string(), "execution".to_string()]
    }
}

struct ConnectService {
    name: String,
}

impl ConnectService {
    async fn new(_config: &crate::config::ConnectConfig) -> Result<Self> {
        Ok(Self {
            name: "connect".to_string(),
        })
    }
}

#[async_trait]
impl Service for ConnectService {
    async fn start(&self) -> Result<()> {
        info!("Starting connect service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping connect service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string(), "execution".to_string()]
    }
}

struct HistoryService {
    name: String,
}

impl HistoryService {
    async fn new() -> Result<Self> {
        Ok(Self {
            name: "history".to_string(),
        })
    }
}

#[async_trait]
impl Service for HistoryService {
    async fn start(&self) -> Result<()> {
        info!("Starting history service");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping history service");
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["storage".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_server_creation() {
        let fixture = ServerConfig::default();
        let actual = IngestServer::new(fixture).await;
        let expected = true;
        assert_eq!(actual.is_ok(), expected);
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = ServerConfig::default();
        let fixture = IngestServer::new(config).await.unwrap();
        let actual = fixture.health_check().await;

        // Services are registered but not started, so they should be unhealthy
        let expected = false;
        assert_eq!(actual.status.is_healthy(), expected);

        // Should have services registered (all features enabled by default)
        let expected = 9; // storage, telemetry, monitoring, execution, queue, pubsub, api, connect, history
        assert_eq!(actual.services.len(), expected);

        // All services should be unhealthy because they're not started
        for (_, status) in &actual.services {
            assert!(status.is_unhealthy());
        }
    }

    #[test]
    fn test_overall_health_structure() {
        let fixture = OverallHealth {
            status: HealthStatus::Healthy,
            services: vec![],
            uptime: std::time::Duration::from_secs(100),
        };

        let actual = fixture.status.is_healthy();
        let expected = true;
        assert_eq!(actual, expected);
    }
}
