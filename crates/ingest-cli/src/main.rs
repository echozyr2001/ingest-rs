use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod config;
mod deploy;
mod dev;
mod utils;

#[derive(Parser)]
#[command(name = "ingest")]
#[command(about = "Inngest CLI for durable function development")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, help = "Enable verbose logging")]
    verbose: bool,

    #[arg(long, global = true, help = "Path to configuration file")]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Inngest project
    Init(cli::init::InitArgs),
    /// Start local development server
    Dev(cli::dev::DevArgs),
    /// Deploy functions to Inngest
    Deploy(cli::deploy::DeployArgs),
    /// Manage functions
    Functions(cli::functions::FunctionArgs),
    /// Manage configuration
    Config(cli::config::ConfigArgs),
    /// Send and manage events
    Events(cli::events::EventArgs),
    /// Generate shell completions
    Completion(cli::completion::CompletionArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose)?;

    // Execute command
    match cli.command {
        Commands::Init(args) => cli::init::execute(args).await,
        Commands::Dev(args) => cli::dev::execute(args).await,
        Commands::Deploy(args) => cli::deploy::execute(args).await,
        Commands::Functions(args) => cli::functions::execute(args).await,
        Commands::Config(args) => cli::config::execute(args).await,
        Commands::Events(args) => cli::events::execute(args).await,
        Commands::Completion(args) => cli::completion::execute(args).await,
    }
}

fn init_logging(verbose: bool) -> Result<()> {
    let filter = if verbose { "debug" } else { "info" };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_cli_parsing() {
        let fixture = vec!["ingest", "init", "my-project"];
        let actual = Cli::try_parse_from(fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_verbose_flag() {
        let fixture = vec!["ingest", "--verbose", "init", "my-project"];
        let actual = Cli::try_parse_from(fixture).unwrap();
        let expected = true;
        assert_eq!(actual.verbose, expected);
    }
}
