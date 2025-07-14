use crate::config::ProjectConfig;
use crate::deploy::DeploymentPipeline;
use crate::utils::output::{error, info, success};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct DeployArgs {
    /// Environment to deploy to
    #[arg(short, long, default_value = "production")]
    pub environment: String,

    /// Deployment configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Dry run - validate without deploying
    #[arg(long)]
    pub dry_run: bool,

    /// Force deployment without confirmation
    #[arg(long)]
    pub force: bool,

    /// Specific function to deploy
    #[arg(long)]
    pub function: Option<String>,
}

pub async fn execute(args: DeployArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let config_path = args
        .config
        .unwrap_or_else(|| project_dir.join("inngest.toml"));

    // Load project configuration
    let config = ProjectConfig::load(&config_path).await?;

    info(&format!("Deploying project: {}", config.project.name));
    info(&format!("Environment: {}", args.environment));

    // Create deployment pipeline
    let pipeline = DeploymentPipeline::new(config, args.environment).await?;

    if args.dry_run {
        info("🔍 Running deployment validation...");
        let validation_result = pipeline.validate().await?;

        if validation_result.is_valid {
            success("✅ Deployment validation passed!");
            println!("Functions to deploy: {}", validation_result.function_count);
        } else {
            error("❌ Deployment validation failed!");
            for error in validation_result.errors {
                println!("  • {error}");
            }
            anyhow::bail!("Validation failed");
        }
        return Ok(());
    }

    // Execute deployment
    info("🚀 Starting deployment...");
    let deployment_result = pipeline.deploy(args.force).await?;

    success("✅ Deployment completed successfully!");
    println!("Deployed functions: {}", deployment_result.deployed_count);
    println!("Deployment URL: {}", deployment_result.url);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_args_parsing() {
        let fixture = DeployArgs {
            environment: "staging".to_string(),
            config: None,
            dry_run: true,
            force: false,
            function: Some("test-function".to_string()),
        };

        let actual = fixture.environment;
        let expected = "staging";
        assert_eq!(actual, expected);
    }
}
