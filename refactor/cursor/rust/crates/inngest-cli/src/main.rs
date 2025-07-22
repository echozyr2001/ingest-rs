#!/usr/bin/env rust
//! Inngest CLI - Reliable workflows, durable execution
//!
//! This is the main entry point for the Inngest CLI, providing commands
//! for starting development servers, production servers, and other utilities.

use clap::{Parser, Subcommand};
use inngest_core::Result;
use std::process;
use tracing::{error, info, Level};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Parser)]
#[command(name = "inngest")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Inngest CLI - Reliable workflows, durable execution")]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(long, short = 'v', global = true)]
    verbose: bool,

    /// Enable debug logging
    #[arg(long, short = 'd', global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the development server
    Dev {
        /// Port to listen on
        #[arg(long, short = 'p', default_value = "8288")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "localhost")]
        host: String,
    },

    /// Start the production server
    Start {
        /// Port to listen on
        #[arg(long, short = 'p', default_value = "8080")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },

    /// Deploy functions
    Deploy {
        /// Path to function definitions
        #[arg(long, short = 'f')]
        functions: Option<String>,
    },

    /// Show version information
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.debug {
        Level::DEBUG
    } else if cli.verbose {
        Level::INFO
    } else {
        Level::WARN
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    let result = match cli.command {
        Commands::Dev { port, host } => {
            info!("Starting Inngest development server on {}:{}", host, port);
            run_dev_server(&host, port).await
        }
        Commands::Start { port, host } => {
            info!("Starting Inngest production server on {}:{}", host, port);
            run_production_server(&host, port).await
        }
        Commands::Deploy { functions } => {
            info!("Deploying functions from: {:?}", functions);
            deploy_functions(functions).await
        }
        Commands::Version => {
            println!("inngest v{}", env!("CARGO_PKG_VERSION"));
            println!(
                "Build: {}",
                std::env::var("VERGEN_GIT_SHA_SHORT").unwrap_or_else(|_| "unknown".to_string())
            );
            Ok(())
        }
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        process::exit(1);
    }
}

async fn run_dev_server(host: &str, port: u16) -> Result<()> {
    // TODO: Implement development server
    info!("Development server would start on {}:{}", host, port);
    info!("Development server not yet implemented");
    Ok(())
}

async fn run_production_server(host: &str, port: u16) -> Result<()> {
    // TODO: Implement production server
    info!("Production server would start on {}:{}", host, port);
    info!("Production server not yet implemented");
    Ok(())
}

async fn deploy_functions(functions_path: Option<String>) -> Result<()> {
    // TODO: Implement function deployment
    info!("Function deployment from {:?}", functions_path);
    info!("Function deployment not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    

    #[test]
    fn test_cli_dev_command() {
        let fixture = vec!["inngest", "dev", "--port", "3000", "--host", "127.0.0.1"];
        let actual = Cli::try_parse_from(fixture);

        assert!(actual.is_ok());

        let cli = actual.unwrap();
        let expected_command =
            matches!(cli.command, Commands::Dev { port: 3000, host } if host == "127.0.0.1");
        assert!(expected_command);
    }

    #[test]
    fn test_cli_start_command() {
        let fixture = vec!["inngest", "start"];
        let actual = Cli::try_parse_from(fixture);

        assert!(actual.is_ok());

        let cli = actual.unwrap();
        let expected_command =
            matches!(cli.command, Commands::Start { port: 8080, host } if host == "0.0.0.0");
        assert!(expected_command);
    }

    #[test]
    fn test_cli_version_command() {
        let fixture = vec!["inngest", "version"];
        let actual = Cli::try_parse_from(fixture);

        assert!(actual.is_ok());

        let cli = actual.unwrap();
        let expected_command = matches!(cli.command, Commands::Version);
        assert!(expected_command);
    }
}
