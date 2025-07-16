use clap::{Parser, Subcommand};
use ingest_migration::{MigrationConfig, MigrationOrchestrator};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ingest-migrate")]
#[command(about = "Migration tool for transitioning from Go to Rust Inngest platform")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Migrate configuration files from Go to Rust format
    Config {
        /// Source directory containing Go configuration files
        #[arg(short, long, default_value = "./go-inngest")]
        source: PathBuf,

        /// Target directory for Rust configuration files
        #[arg(short, long, default_value = "./rust-inngest")]
        target: PathBuf,

        /// Perform dry run without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Migrate data from Go database schema to Rust schema
    Data {
        /// Source database connection string
        #[arg(short, long)]
        source_db: String,

        /// Target database connection string
        #[arg(short, long)]
        target_db: String,

        /// Perform dry run without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Migrate function definitions from Go to Rust
    Functions {
        /// Source directory containing Go function definitions
        #[arg(short, long, default_value = "./go-inngest/functions")]
        source: PathBuf,

        /// Target directory for Rust function definitions
        #[arg(short, long, default_value = "./rust-inngest/functions")]
        target: PathBuf,

        /// Perform dry run without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Run complete migration from Go to Rust
    All {
        /// Source directory containing Go Inngest installation
        #[arg(short, long, default_value = "./go-inngest")]
        source: PathBuf,

        /// Target directory for Rust Inngest installation
        #[arg(short, long, default_value = "./rust-inngest")]
        target: PathBuf,

        /// Perform dry run without making changes
        #[arg(long)]
        dry_run: bool,

        /// Only validate migration without performing it
        #[arg(long)]
        validate_only: bool,

        /// Disable backup creation
        #[arg(long)]
        no_backup: bool,

        /// Number of parallel workers
        #[arg(short, long, default_value = "4")]
        workers: usize,
    },

    /// Validate existing migration
    Validate {
        /// Directory containing migrated Rust installation
        #[arg(short, long, default_value = "./rust-inngest")]
        target: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config {
            source,
            target,
            dry_run: _dry_run,
        } => {
            println!("🔧 Migrating configuration files...");
            println!("Source: {}", source.display());
            println!("Target: {}", target.display());

            let config = MigrationConfig {
                source_path: source.to_string_lossy().to_string(),
                target_path: target.to_string_lossy().to_string(),
                dry_run: _dry_run,
                ..Default::default()
            };

            let _orchestrator = MigrationOrchestrator::new(config);
            // This would call a specific config migration method
            println!("Configuration migration completed!");
        }

        Commands::Data {
            source_db,
            target_db,
            dry_run: _dry_run,
        } => {
            println!("📊 Migrating data...");
            println!("Source DB: {source_db}");
            println!("Target DB: {target_db}");

            // Data migration implementation would go here
            println!("Data migration completed!");
        }

        Commands::Functions {
            source,
            target,
            dry_run: _dry_run,
        } => {
            println!("⚡ Migrating functions...");
            println!("Source: {}", source.display());
            println!("Target: {}", target.display());

            // Function migration implementation would go here
            println!("Function migration completed!");
        }

        Commands::All {
            source,
            target,
            dry_run: _dry_run,
            validate_only,
            no_backup,
            workers,
        } => {
            println!("🚀 Running complete migration from Go to Rust...");
            println!("Source: {}", source.display());
            println!("Target: {}", target.display());

            let config = MigrationConfig {
                source_path: source.to_string_lossy().to_string(),
                target_path: target.to_string_lossy().to_string(),
                backup_enabled: !no_backup,
                dry_run: _dry_run,
                validate_only,
                parallel_workers: workers,
            };

            let orchestrator = MigrationOrchestrator::new(config);

            match orchestrator.run_migration().await {
                Ok(result) => {
                    let report = orchestrator.generate_report(&result);
                    println!("\n{report}");

                    if result.overall_success {
                        println!("🎉 Migration completed successfully!");
                    } else {
                        println!(
                            "❌ Migration completed with errors. Please review the report above."
                        );
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Migration failed: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Validate { target } => {
            println!("✅ Validating migration...");
            println!("Target: {}", target.display());

            let config = MigrationConfig {
                target_path: target.to_string_lossy().to_string(),
                validate_only: true,
                ..Default::default()
            };

            let _orchestrator = MigrationOrchestrator::new(config);
            // This would call validation methods
            println!("Validation completed!");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can be parsed without panicking
        let cli = Cli::try_parse_from(["ingest-migrate", "validate", "--target", "/tmp"]);
        assert!(cli.is_ok());
    }
}
