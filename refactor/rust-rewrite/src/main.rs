use anyhow::Result;
use clap::{Parser, Subcommand};
use inngest_cli::{DevCommand, StartCommand, VersionCommand};

#[derive(Parser)]
#[command(name = "inngest")]
#[command(about = "Inngest CLI - The durable execution engine with built-in flow control")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true)]
    verbose: bool,

    #[arg(long, global = true, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the Inngest Dev Server for local development
    Dev(DevCommand),
    /// Start the Inngest server for production
    Start(StartCommand),
    /// Show version information
    Version(VersionCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level, cli.verbose)?;

    match cli.command {
        Commands::Dev(cmd) => cmd.execute().await,
        Commands::Start(cmd) => cmd.execute().await,
        Commands::Version(cmd) => cmd.execute().await,
    }
}

fn init_logging(level: &str, verbose: bool) -> Result<()> {
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let level = if verbose { "debug" } else { level };

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new(level))
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
