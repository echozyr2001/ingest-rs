use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Inngest Server - Durable Functions Platform
#[derive(Parser, Debug)]
#[command(name = "ingest-server")]
#[command(about = "A high-performance durable functions platform")]
#[command(version)]
pub struct Args {
    /// Configuration file path
    #[arg(short, long, env = "INGEST_CONFIG_FILE")]
    pub config: Option<PathBuf>,

    /// Server bind address
    #[arg(long, env = "INGEST_BIND_ADDRESS", default_value = "0.0.0.0")]
    pub bind_address: String,

    /// Server port
    #[arg(short, long, env = "INGEST_PORT", default_value = "8080")]
    pub port: u16,

    /// Log level
    #[arg(long, env = "INGEST_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Enable development mode
    #[arg(long, env = "INGEST_DEV_MODE")]
    pub dev_mode: bool,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,

    /// Redis URL
    #[arg(long, env = "REDIS_URL")]
    pub redis_url: Option<String>,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Start the server
    Start {
        /// Run in foreground
        #[arg(long)]
        foreground: bool,
    },
    /// Check server health
    Health {
        /// Health check endpoint URL
        #[arg(long, default_value = "http://localhost:8080/health")]
        url: String,
    },
    /// Validate configuration
    Config {
        /// Show resolved configuration
        #[arg(long)]
        show: bool,
    },
    /// Run database migrations
    Migrate {
        /// Migration direction
        #[arg(long, default_value = "up")]
        direction: String,
    },
    /// Generate default configuration
    Init {
        /// Output file path
        #[arg(short, long, default_value = "ingest.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },
}

impl Args {
    /// Get the effective configuration file path
    pub fn config_file(&self) -> Option<&PathBuf> {
        self.config.as_ref()
    }

    /// Get the server address
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }

    /// Check if running in development mode
    pub fn is_dev_mode(&self) -> bool {
        self.dev_mode
    }

    /// Get the command to execute
    pub fn command(&self) -> &Option<Commands> {
        &self.command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_args() {
        let fixture = Args::parse_from(["ingest-server"]);
        let actual = fixture.server_address();
        let expected = "0.0.0.0:8080";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_custom_port() {
        let fixture = Args::parse_from(["ingest-server", "--port", "9000"]);
        let actual = fixture.port;
        let expected = 9000;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_dev_mode_flag() {
        let fixture = Args::parse_from(["ingest-server", "--dev-mode"]);
        let actual = fixture.is_dev_mode();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_start_command() {
        let fixture = Args::parse_from(["ingest-server", "start", "--foreground"]);
        let actual = matches!(fixture.command, Some(Commands::Start { foreground: true }));
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_config_command() {
        let fixture = Args::parse_from(["ingest-server", "config", "--show"]);
        let actual = matches!(fixture.command, Some(Commands::Config { show: true }));
        let expected = true;
        assert_eq!(actual, expected);
    }
}
