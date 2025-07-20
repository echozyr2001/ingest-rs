pub mod api;
pub mod production_server;
pub mod production_state;

pub use production_server::*;

use anyhow::Result;
use clap::Parser;
use std::net::IpAddr;

/// Production server command
#[derive(Parser, Debug)]
pub struct StartCommand {
    /// Port to serve on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    pub host: IpAddr,

    /// Database URL (overrides config/env)
    #[arg(long)]
    pub database_url: Option<String>,

    /// Redis URL (overrides config/env)
    #[arg(long)]
    pub redis_url: Option<String>,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

impl StartCommand {
    pub async fn execute(&self) -> Result<()> {
        use inngest_config::Config;

        tracing::info!(
            "Starting Inngest production server on {}:{}",
            self.host,
            self.port
        );

        // Load configuration
        let mut config = if let Some(config_path) = &self.config {
            // Load from file
            let content = std::fs::read_to_string(config_path)?;
            serde_yaml::from_str(&content)?
        } else {
            // Load from environment
            Config::from_env()?
        };

        // Override with command line arguments
        if let Some(database_url) = &self.database_url {
            config.database.url = database_url.clone();
        }
        if let Some(redis_url) = &self.redis_url {
            config.redis.url = redis_url.clone();
        }
        config.api.addr = self.host.to_string();
        config.api.port = self.port;

        // Start production server
        start_production_server(config).await
    }
}

/// Development server command
#[derive(Parser, Debug)]
pub struct DevCommand {
    /// URLs to discover and sync functions from
    #[arg(short, long, value_name = "URL")]
    pub urls: Vec<String>,

    /// Port to serve the dev server on
    #[arg(short, long, default_value = "8288")]
    pub port: u16,

    /// Host to bind the dev server to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: IpAddr,

    /// Disable automatic function discovery
    #[arg(long)]
    pub no_discovery: bool,

    /// Disable polling for function changes
    #[arg(long)]
    pub no_poll: bool,

    /// Polling interval in milliseconds
    #[arg(long, default_value = "1000")]
    pub poll_interval: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

impl DevCommand {
    pub async fn execute(&self) -> Result<()> {
        tracing::info!("Starting Inngest dev server on {}:{}", self.host, self.port);

        let config = inngest_devserver::DevServerConfig {
            host: self.host,
            port: self.port,
            urls: self.urls.clone(),
            discovery_enabled: !self.no_discovery,
            polling_enabled: !self.no_poll,
            poll_interval_ms: self.poll_interval,
        };

        let server = inngest_devserver::DevServer::new(config).await?;
        server.start().await
    }
}

/// Version command
#[derive(Parser, Debug)]
pub struct VersionCommand;

impl VersionCommand {
    pub async fn execute(&self) -> Result<()> {
        println!("Inngest Rust {}", env!("CARGO_PKG_VERSION"));
        println!("Built from commit: {}", "unknown"); // TODO: Add vergen build script
        Ok(())
    }
}

/// Start production server with full PostgreSQL, Redis, and queue consumer
async fn start_production_server(config: inngest_config::Config) -> Result<()> {
    use crate::production_server::ProductionServer;
    use tokio::signal;

    tracing::info!("Initializing Phase 5 production server with execution engine...");

    // Create production server with all components
    let mut server = ProductionServer::new(
        &config.database.url,
        &config.redis.url,
        "http://localhost:3000".to_string(), // Default function base URL
    )
    .await?;

    tracing::info!("Starting production server with queue consumer...");

    // Save values before moving into spawn
    let addr = config.api.addr.clone();
    let port = config.api.port;

    // Start the server (includes API + queue consumer)
    tokio::spawn(async move {
        if let Err(e) = server.start(&config.api.addr, config.api.port).await {
            tracing::error!("Server failed: {}", e);
        }
    });

    tracing::info!("🚀 Phase 5 Production Server running on {}:{}", addr, port);
    tracing::info!("📊 API endpoints available:");
    tracing::info!("  - GET  /api/v1/functions");
    tracing::info!("  - POST /api/v1/functions");
    tracing::info!("  - POST /api/v1/events");
    tracing::info!("  - GET  /api/v1/runs");
    tracing::info!("  - GET  /health");
    tracing::info!("⚡ Queue consumer processing function executions");

    // Wait for shutdown signal
    signal::ctrl_c().await?;
    tracing::info!("Received shutdown signal, stopping server...");

    // Note: Graceful shutdown would require keeping server handle
    tracing::info!("Server shutdown requested");

    Ok(())
}
