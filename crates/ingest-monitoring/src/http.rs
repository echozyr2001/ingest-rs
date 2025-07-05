use crate::error::{MonitoringError, Result};
use crate::health::{HealthMonitor, HealthStatus};
use axum::{Router, extract::State, http::StatusCode, response::Json, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

/// Health server response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall status
    pub status: HealthStatus,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Individual check results
    pub checks: Vec<HealthCheckResponse>,
    /// Service information
    pub service: ServiceInfo,
}

/// Individual health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    /// Name of the component
    pub name: String,
    /// Status of the component
    pub status: HealthStatus,
    /// Optional message
    pub message: Option<String>,
    /// Duration of the check in milliseconds
    pub duration_ms: u64,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Service information
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Service name
    pub name: String,
    /// Service version
    pub version: String,
    /// Build timestamp
    pub build_time: String,
    /// Git commit hash
    pub git_hash: String,
}

impl Default for ServiceInfo {
    fn default() -> Self {
        Self {
            name: "inngest-ingest".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: chrono::Utc::now().to_rfc3339(),
            git_hash: "unknown".to_string(),
        }
    }
}

/// HTTP health server
pub struct HealthServer {
    port: u16,
    health_monitor: Arc<HealthMonitor>,
    server_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl HealthServer {
    /// Create a new health server
    pub fn new(port: u16, health_monitor: Arc<HealthMonitor>) -> Self {
        Self {
            port,
            health_monitor,
            server_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the health server
    pub async fn start(&self) -> Result<()> {
        let mut handle = self.server_handle.write().await;
        if handle.is_some() {
            return Err(MonitoringError::http("Health server already running"));
        }

        let app = self.create_app().await;
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| {
                MonitoringError::http(format!("Failed to bind to port {}: {}", self.port, e))
            })?;

        println!("Health server starting on port {}", self.port);

        let server_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("Health server error: {e}");
            }
        });

        *handle = Some(server_handle);
        Ok(())
    }

    /// Stop the health server
    pub async fn stop(&self) -> Result<()> {
        let mut handle = self.server_handle.write().await;
        if let Some(server_handle) = handle.take() {
            server_handle.abort();
            println!("Health server stopped");
        }
        Ok(())
    }

    /// Create the Axum app
    async fn create_app(&self) -> Router {
        let health_monitor = self.health_monitor.clone();

        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .with_state(health_monitor)
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
    }
}

/// Health check handler
async fn health_handler(
    State(health_monitor): State<Arc<HealthMonitor>>,
) -> axum::response::Result<Json<HealthResponse>, StatusCode> {
    let status = health_monitor.overall_health().await;
    let details = health_monitor.health_details().await;

    let checks = details
        .into_iter()
        .map(|check| HealthCheckResponse {
            name: check.name,
            status: check.status,
            message: check.message,
            duration_ms: check.duration.as_millis() as u64,
            metadata: check.metadata,
        })
        .collect();

    let response = HealthResponse {
        status: status.clone(),
        timestamp: chrono::Utc::now(),
        checks,
        service: ServiceInfo::default(),
    };

    // Return appropriate HTTP status code
    let status_code = match status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still return 200 for degraded
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    match status_code {
        StatusCode::OK => Ok(Json(response)),
        _ => Err(status_code),
    }
}

/// Liveness probe handler (always returns 200 if the service is running)
async fn liveness_handler() -> StatusCode {
    StatusCode::OK
}

/// Readiness probe handler (returns 200 only if the service is ready to serve traffic)
async fn readiness_handler(State(health_monitor): State<Arc<HealthMonitor>>) -> StatusCode {
    let status = health_monitor.overall_health().await;
    match status {
        HealthStatus::Healthy | HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_service_info_default() {
        let info = ServiceInfo::default();
        assert_eq!(info.name, "inngest-ingest");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.git_hash, "unknown");
    }

    #[test]
    fn test_health_server_creation() {
        let health_monitor = Arc::new(HealthMonitor::new());
        let server = HealthServer::new(8080, health_monitor);
        assert_eq!(server.port, 8080);
    }

    #[tokio::test]
    async fn test_health_server_lifecycle() {
        let health_monitor = Arc::new(HealthMonitor::new());
        let server = HealthServer::new(0, health_monitor); // Use port 0 for testing

        // Test that server can be created and stopped without starting
        let result = server.stop().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_health_check_response() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        let response = HealthCheckResponse {
            name: "test".to_string(),
            status: HealthStatus::Healthy,
            message: Some("All good".to_string()),
            duration_ms: 100,
            metadata,
        };

        assert_eq!(response.name, "test");
        assert_eq!(response.status, HealthStatus::Healthy);
        assert_eq!(response.message, Some("All good".to_string()));
        assert_eq!(response.duration_ms, 100);
    }

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: HealthStatus::Healthy,
            timestamp: chrono::Utc::now(),
            checks: vec![],
            service: ServiceInfo::default(),
        };

        assert_eq!(response.status, HealthStatus::Healthy);
        assert!(response.checks.is_empty());
    }
}
