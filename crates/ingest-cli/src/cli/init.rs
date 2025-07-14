use crate::utils::output::success;
use anyhow::Result;
use clap::Args;
use dialoguer::{Confirm, Input};
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
pub struct InitArgs {
    /// Project name
    #[arg(value_name = "NAME")]
    pub name: Option<String>,

    /// Project template
    #[arg(long, default_value = "basic")]
    pub template: String,

    /// Programming language
    #[arg(long, default_value = "rust")]
    pub language: String,

    /// Skip interactive prompts
    #[arg(long)]
    pub no_interactive: bool,

    /// Target directory
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

pub async fn execute(args: InitArgs) -> Result<()> {
    let project_name = get_project_name(args.name, args.no_interactive)?;
    let target_dir = args.dir.unwrap_or_else(|| PathBuf::from(&project_name));

    if target_dir.exists() && !args.no_interactive {
        let overwrite = Confirm::new()
            .with_prompt(format!(
                "Directory '{}' already exists. Continue?",
                target_dir.display()
            ))
            .interact()?;

        if !overwrite {
            return Ok(());
        }
    }

    create_project_structure(&target_dir, &project_name, &args.template).await?;

    success(&format!(
        "✨ Project '{project_name}' created successfully!"
    ));
    println!("\nNext steps:");
    println!("  cd {project_name}");
    println!("  ingest dev");

    Ok(())
}

fn get_project_name(name: Option<String>, no_interactive: bool) -> Result<String> {
    match name {
        Some(name) => Ok(name),
        None if no_interactive => {
            anyhow::bail!("Project name is required when using --no-interactive")
        }
        None => {
            let name: String = Input::new().with_prompt("Project name").interact_text()?;
            Ok(name)
        }
    }
}

async fn create_project_structure(dir: &PathBuf, name: &str, _template: &str) -> Result<()> {
    // Create directory structure
    fs::create_dir_all(dir)?;
    fs::create_dir_all(dir.join("src").join("functions"))?;
    fs::create_dir_all(dir.join("events"))?;

    // Create inngest.toml configuration
    let config = format!(
        r#"[project]
name = "{name}"
version = "0.1.0"
functions_dir = "src/functions"
build_dir = "target/inngest"

[inngest]
app_id = "{name}"
serve_path = "/api/inngest"

[dev]
port = 3000
auto_reload = true

[deploy]
environment = "development"
"#
    );

    fs::write(dir.join("inngest.toml"), config)?;

    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
inngest = "0.1"
tokio = {{ version = "1.0", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
anyhow = "1.0"

[[bin]]
name = "server"
path = "src/main.rs"
"#
    );

    fs::write(dir.join("Cargo.toml"), cargo_toml)?;

    // Create main.rs
    let main_rs = r#"use anyhow::Result;
use inngest::{Inngest, serve};

mod functions;

#[tokio::main]
async fn main() -> Result<()> {
    let inngest = Inngest::new("my-app");
    
    // Register functions
    let functions = functions::register_all();
    
    // Start server
    serve(inngest, functions).await
}
"#;

    fs::write(dir.join("src").join("main.rs"), main_rs)?;

    // Create example function
    let function_rs = r#"use inngest::{Function, FunctionOpts, Event};
use serde_json::Value;
use anyhow::Result;

pub fn register_all() -> Vec<Function> {
    vec![
        hello_world(),
    ]
}

fn hello_world() -> Function {
    Function::new(
        FunctionOpts::new("hello-world")
            .trigger("app/hello"),
        |event: Event, _step| async move {
            println!("Hello, {}!", event.data.get("name").unwrap_or(&Value::String("World".to_string())));
            Ok(())
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    
    #[test]
    fn test_function_registration() {
        let fixture = register_all();
        let actual = fixture.len();
        let expected = 1;
        assert_eq!(actual, expected);
    }
}
"#;

    fs::write(
        dir.join("src").join("functions").join("mod.rs"),
        function_rs,
    )?;

    // Create README.md
    let readme = format!(
        r#"# {name}

An Inngest project for durable function execution.

## Getting Started

1. Start the development server:
   ```bash
   ingest dev
   ```

2. Send a test event:
   ```bash
   ingest events send app/hello '{{"name": "World"}}'
   ```

## Project Structure

- `src/functions/` - Function definitions
- `events/` - Event schemas and examples
- `inngest.toml` - Project configuration

## Commands

- `ingest dev` - Start development server
- `ingest deploy` - Deploy to production
- `ingest functions list` - List all functions
- `ingest events send <event> <data>` - Send test event
"#
    );

    fs::write(dir.join("README.md"), readme)?;

    // Create .gitignore
    let gitignore = r#"target/
.env
.DS_Store
*.log
"#;

    fs::write(dir.join(".gitignore"), gitignore)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_project_creation() {
        let fixture = TempDir::new().unwrap();
        let project_name = "test-project";
        let actual =
            create_project_structure(&fixture.path().to_path_buf(), project_name, "basic").await;
        assert!(actual.is_ok());

        let expected = true;
        let config_exists = fixture.path().join("inngest.toml").exists();
        assert_eq!(config_exists, expected);
    }

    #[test]
    fn test_project_name_interactive() {
        let fixture = Some("my-project".to_string());
        let actual = get_project_name(fixture, false).unwrap();
        let expected = "my-project";
        assert_eq!(actual, expected);
    }
}
