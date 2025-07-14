// Import the main Cli struct from main.rs
// Note: We'll define a local Cli struct for completion generation
use crate::utils::output::{info, success};
use anyhow::Result;
use clap::{Args, Command};
use clap_complete::{Shell, generate};
use std::io;

#[derive(Args)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub async fn execute(args: CompletionArgs) -> Result<()> {
    info(&format!(
        "🔧 Generating shell completions for {:?}...",
        args.shell
    ));

    // Create a command for completion generation
    let mut cmd = Command::new("ingest")
        .about("Inngest CLI for durable function development")
        .version("0.1.0");

    generate(args.shell, &mut cmd, "ingest", &mut io::stdout());

    success("Shell completions generated!");
    println!();

    match args.shell {
        Shell::Bash => {
            println!("To enable completions, add this to your ~/.bashrc:");
            println!("  source <(ingest completion bash)");
        }
        Shell::Zsh => {
            println!("To enable completions, add this to your ~/.zshrc:");
            println!("  source <(ingest completion zsh)");
        }
        Shell::Fish => {
            println!("To enable completions, run:");
            println!("  ingest completion fish | source");
        }
        Shell::PowerShell => {
            println!("To enable completions, add this to your PowerShell profile:");
            println!("  Invoke-Expression (ingest completion powershell | Out-String)");
        }
        _ => {
            println!("Completions generated for {:?}", args.shell);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_completion_generation() {
        let fixture = CompletionArgs { shell: Shell::Bash };

        let actual = execute(fixture).await;
        assert!(actual.is_ok());
    }
}
