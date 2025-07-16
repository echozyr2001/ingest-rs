//! API server implementation

use crate::{
    Result, config::ApiConfig, error::ApiError, handlers::AppState, routes::create_router,
};
use axum::Router;
use std::{net::SocketAddr, time::Instant};
use tokio::net::TcpListener;
use tracing::info;

/// Main API server
pub struct ApiServer {
    config: ApiConfig,
    app: Router,
    start_time: Instant,
}

impl ApiServer {
    /// Create a new API server
    pub async fn new(config: ApiConfig) -> Result<Self> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Config validation failed: {}", e)))?;

        info!(
            bind_address = %config.bind_address,
            port = config.port,
            "Initializing API server"
        );

        // Create application state
        let app_state = Self::create_app_state().await?;

        // Create router
        let app = create_router(app_state, &config);

        Ok(Self {
            config,
            app,
            start_time: Instant::now(),
        })
    }

    /// Create application state with all required services
    async fn create_app_state() -> Result<AppState> {
        info!("Initializing application services");

        Ok(AppState {
            start_time: Instant::now(),
        })
    }

    /// Start the server
    pub async fn serve(self, addr: &str) -> Result<()> {
        let socket_addr: SocketAddr = addr.parse().map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Invalid address '{}': {}", addr, e))
        })?;

        info!(
            address = %socket_addr,
            uptime = ?self.start_time.elapsed(),
            "Starting API server"
        );

        // Create TCP listener
        let listener = TcpListener::bind(&socket_addr).await.map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to bind to {}: {}", socket_addr, e))
        })?;

        info!(
            address = %socket_addr,
            "API server listening"
        );

        // Start the server
        axum::serve(listener, self.app)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Server error: {}", e)))?;

        Ok(())
    }

    /// Get server configuration
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }

    /// Get server uptime
    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Create a test server for testing purposes
    #[cfg(test)]
    pub async fn test_server() -> Result<Self> {
        let config = ApiConfig::default();
        Self::new(config).await
    }

    /// Create an in-memory API server for benchmarking
    pub async fn new_in_memory() -> Result<Self> {
        let config = ApiConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 0, // Use random port for testing
            ..Default::default()
        };
        Self::new(config).await
    }
}

/// Builder for creating API servers with custom configuration
pub struct ApiServerBuilder {
    config: ApiConfig,
}

impl ApiServerBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: ApiConfig::default(),
        }
    }

    /// Set the bind address
    pub fn bind_address(mut self, addr: impl Into<String>) -> Self {
        self.config.bind_address = addr.into();
        self
    }

    /// Set the port
    pub fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    /// Set the request timeout
    pub fn request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.request_timeout = timeout;
        self
    }

    /// Build the API server
    pub async fn build(self) -> Result<ApiServer> {
        ApiServer::new(self.config).await
    }
}

impl Default for ApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    #[tokio::test]
    async fn test_server_creation() {
        let server = ApiServer::test_server().await.unwrap();

        assert_eq!(server.config.bind_address, "0.0.0.0");
        assert_eq!(server.config.port, 3000);
        // Ensure uptime is non-negative (trivially true, so just check it's less than some reasonable value)
        assert!(server.uptime().as_millis() < 10_000);
    }

    #[test]
    fn test_server_builder() {
        let builder = ApiServerBuilder::new()
            .bind_address("127.0.0.1")
            .port(8080)
            .request_timeout(Duration::from_secs(60));

        assert_eq!(builder.config.bind_address, "127.0.0.1");
        assert_eq!(builder.config.port, 8080);
        assert_eq!(builder.config.request_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_builder_default() {
        let builder = ApiServerBuilder::default();

        assert_eq!(builder.config.bind_address, "0.0.0.0");
        assert_eq!(builder.config.port, 3000);
    }

    #[tokio::test]
    async fn test_invalid_address() {
        let config = ApiConfig {
            bind_address: "invalid-address".to_string(),
            ..Default::default()
        };

        let server = ApiServer::new(config).await.unwrap();
        let result = server.serve("invalid-address:invalid-port").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_app_state_creation() {
        let app_state = ApiServer::create_app_state().await.unwrap();

        // Verify state is created by checking that start_time is not in the future
        assert!(app_state.start_time <= Instant::now());
    }
}
