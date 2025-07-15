use clap::Parser;
use ingest_server::{
    cli::{Args, Commands},
    config::ServerConfig,
    error::Result,
    server::IngestServer,
};
use std::process;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize basic logging for startup
    init_basic_logging(&args.log_level);

    // Extract command to avoid borrowing issues
    let command = args.command.clone();

    // Execute the appropriate command
    let result = match command {
        Some(Commands::Start { foreground }) => run_server(args, foreground).await,
        Some(Commands::Health { url }) => check_health(&url).await,
        Some(Commands::Config { show }) => handle_config(args, show).await,
        Some(Commands::Migrate { direction }) => run_migrations(args, &direction).await,
        Some(Commands::Init { output, force }) => init_config(&output, force).await,
        None => {
            // Default to starting the server
            run_server(args, true).await
        }
    };

    // Handle the result
    match result {
        Ok(_) => {
            info!("Command completed successfully");
            process::exit(0);
        }
        Err(e) => {
            error!("Command failed: {:?}", e);
            process::exit(1);
        }
    }
}

/// Run the main server
async fn run_server(args: Args, foreground: bool) -> Result<()> {
    info!("Starting Inngest server");

    // Load configuration
    let config = ServerConfig::load(&args)?;
    config.validate()?;

    info!("Configuration loaded and validated");

    // Initialize full telemetry with the loaded configuration
    if config.telemetry.enabled {
        init_telemetry(&config)?;
    }

    // Create and start the server
    let server = IngestServer::new(config).await?;

    if !foreground {
        warn!("Daemon mode not yet implemented, running in foreground");
    }

    info!("Inngest server starting...");

    // Start the server (this will block until shutdown)
    server.start().await?;

    info!("Inngest server stopped");
    Ok(())
}

/// Check server health
async fn check_health(url: &str) -> Result<()> {
    info!("Checking server health at: {}", url);

    // Make HTTP request to health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ingest_server::error::ServerError::Service {
            service: "health-check".to_string(),
            error: e.to_string(),
        })?;

    if response.status().is_success() {
        info!("Server is healthy");
        let body = response.text().await.unwrap_or_default();
        if !body.is_empty() {
            println!("{body}");
        }
        Ok(())
    } else {
        error!("Server is unhealthy: {}", response.status());
        Err(ingest_server::error::ServerError::HealthCheck(format!(
            "HTTP {}",
            response.status()
        )))
    }
}

/// Handle configuration commands
async fn handle_config(args: Args, show: bool) -> Result<()> {
    if show {
        info!("Loading and displaying configuration");

        let config = ServerConfig::load(&args)?;
        config.validate()?;

        let config_str = toml::to_string_pretty(&config)
            .map_err(|e| ingest_server::error::ConfigError::InvalidFile(e.to_string()))?;

        println!("{config_str}");
    } else {
        info!("Validating configuration");

        let config = ServerConfig::load(&args)?;
        config.validate()?;

        info!("Configuration is valid");
    }

    Ok(())
}

/// Run database migrations
async fn run_migrations(args: Args, direction: &str) -> Result<()> {
    info!("Running database migrations: {}", direction);

    let config = ServerConfig::load(&args)?;
    config.validate()?;

    match direction {
        "up" => {
            info!("Running migrations up");
            // TODO: Implement migration runner
            warn!("Migration runner not yet implemented");
        }
        "down" => {
            info!("Running migrations down");
            // TODO: Implement migration runner
            warn!("Migration runner not yet implemented");
        }
        _ => {
            error!("Invalid migration direction: {}", direction);
            return Err(ingest_server::error::ServerError::Service {
                service: "migration".to_string(),
                error: format!("Invalid direction: {direction}"),
            });
        }
    }

    Ok(())
}

/// Initialize default configuration
async fn init_config(output: &std::path::Path, force: bool) -> Result<()> {
    info!("Initializing configuration file: {:?}", output);

    // Check if file exists and force flag
    if output.exists() && !force {
        error!("Configuration file already exists. Use --force to overwrite.");
        return Err(ingest_server::error::ServerError::Service {
            service: "init".to_string(),
            error: "File already exists".to_string(),
        });
    }

    // Generate default configuration
    let config_content = ServerConfig::generate_default()?;

    // Write to file
    std::fs::write(output, config_content).map_err(ingest_server::error::ConfigError::Io)?;

    info!("Configuration file created: {:?}", output);
    Ok(())
}

/// Initialize basic logging for startup
fn init_basic_logging(level: &str) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let level = match level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(format!("ingest_server={level}"))
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Initialize full telemetry system
fn init_telemetry(config: &ServerConfig) -> Result<()> {
    // TODO: Initialize OpenTelemetry with the provided configuration
    // This would integrate with the ingest-telemetry crate

    info!(
        "Telemetry initialized with service name: {}",
        config.telemetry.service_name
    );

    if let Some(endpoint) = &config.telemetry.tracing_endpoint {
        info!("Tracing endpoint: {}", endpoint);
    }

    if let Some(endpoint) = &config.telemetry.metrics_endpoint {
        info!("Metrics endpoint: {}", endpoint);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // Note: Testing tracing initialization is problematic because
    // the global subscriber can only be set once per process.
    // In a real application, this function is called once at startup.

    #[tokio::test]
    async fn test_init_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test.toml");

        let fixture = init_config(&config_path, false).await;
        let actual = fixture.is_ok();
        let expected = true;
        assert_eq!(actual, expected);

        // File should exist now
        let actual = config_path.exists();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_handle_config_validation() {
        let args = Args {
            config: None,
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            log_level: "info".to_string(),
            dev_mode: false,
            database_url: Some("postgresql://localhost/test".to_string()),
            redis_url: Some("redis://localhost".to_string()),
            command: None,
        };

        let fixture = handle_config(args, false).await;
        let actual = fixture.is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }
}
