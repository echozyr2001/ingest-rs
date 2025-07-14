use crate::deploy::pipeline::FunctionDefinition;
use anyhow::Result;

pub struct ValidationResult {
    pub is_valid: bool,
    pub function_count: usize,
    #[allow(dead_code)]
    pub valid_count: usize,
    pub errors: Vec<String>,
}

pub struct FunctionValidator {
    // TODO: Add expression engine and schema validator
}

impl FunctionValidator {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn validate_function(&self, function: &FunctionDefinition) -> Result<()> {
        // Validate function name
        self.validate_function_name(&function.name)?;

        // Validate trigger expression
        self.validate_trigger(&function.trigger)?;

        // Validate function file exists
        self.validate_function_file(&function.file_path).await?;

        // TODO: Add more validation logic
        // - Parse function code
        // - Validate function signature
        // - Check dependencies
        // - Validate step definitions

        Ok(())
    }

    fn validate_function_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            anyhow::bail!("Function name cannot be empty");
        }

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!(
                "Function name can only contain alphanumeric characters, hyphens, and underscores"
            );
        }

        if name.len() > 64 {
            anyhow::bail!("Function name cannot be longer than 64 characters");
        }

        Ok(())
    }

    fn validate_trigger(&self, trigger: &str) -> Result<()> {
        if trigger.is_empty() {
            anyhow::bail!("Function trigger cannot be empty");
        }

        // Basic trigger format validation
        if !trigger.contains('/') {
            anyhow::bail!("Trigger must be in format 'namespace/event'");
        }

        let parts: Vec<&str> = trigger.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Trigger must have exactly one '/' separator");
        }

        let (namespace, event) = (parts[0], parts[1]);

        if namespace.is_empty() || event.is_empty() {
            anyhow::bail!("Both namespace and event name must be non-empty");
        }

        // Validate characters
        for part in &[namespace, event] {
            if !part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                anyhow::bail!(
                    "Trigger parts can only contain alphanumeric characters, hyphens, underscores, and dots"
                );
            }
        }

        Ok(())
    }

    async fn validate_function_file(&self, file_path: &std::path::Path) -> Result<()> {
        if !file_path.exists() {
            anyhow::bail!("Function file does not exist: {}", file_path.display());
        }

        if !file_path.is_file() {
            anyhow::bail!("Function path is not a file: {}", file_path.display());
        }

        // Check file extension
        if let Some(extension) = file_path.extension() {
            if extension != "rs" {
                anyhow::bail!("Function file must have .rs extension");
            }
        } else {
            anyhow::bail!("Function file must have .rs extension");
        }

        // TODO: Parse and validate Rust code
        // - Check for required function signature
        // - Validate imports
        // - Check for syntax errors

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn validate_project_structure(&self, project_dir: &std::path::Path) -> Result<()> {
        // Check for required files
        let cargo_toml = project_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            anyhow::bail!("Cargo.toml not found in project directory");
        }

        let inngest_toml = project_dir.join("inngest.toml");
        if !inngest_toml.exists() {
            anyhow::bail!("inngest.toml not found in project directory");
        }

        // TODO: Add more project structure validation

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_validate_function_name() {
        let fixture = FunctionValidator::new().unwrap();

        // Valid names
        assert!(fixture.validate_function_name("hello-world").is_ok());
        assert!(fixture.validate_function_name("process_order").is_ok());
        assert!(fixture.validate_function_name("send123").is_ok());

        // Invalid names
        assert!(fixture.validate_function_name("").is_err());
        assert!(fixture.validate_function_name("hello world").is_err());
        assert!(fixture.validate_function_name("hello@world").is_err());
        assert!(fixture.validate_function_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn test_validate_trigger() {
        let fixture = FunctionValidator::new().unwrap();

        // Valid triggers
        assert!(fixture.validate_trigger("app/user.created").is_ok());
        assert!(fixture.validate_trigger("payment/order-completed").is_ok());
        assert!(fixture.validate_trigger("email/sent").is_ok());

        // Invalid triggers
        assert!(fixture.validate_trigger("").is_err());
        assert!(fixture.validate_trigger("no-slash").is_err());
        assert!(fixture.validate_trigger("too/many/slashes").is_err());
        assert!(fixture.validate_trigger("/empty-namespace").is_err());
        assert!(fixture.validate_trigger("empty-event/").is_err());
        assert!(fixture.validate_trigger("invalid@chars/event").is_err());
    }

    #[tokio::test]
    async fn test_validate_function_file() {
        let fixture = FunctionValidator::new().unwrap();

        // Create a temporary .rs file
        let temp_file = NamedTempFile::with_suffix(".rs").unwrap();
        let actual = fixture.validate_function_file(temp_file.path()).await;
        assert!(actual.is_ok());

        // Test non-existent file
        let non_existent = PathBuf::from("non_existent.rs");
        let actual = fixture.validate_function_file(&non_existent).await;
        assert!(actual.is_err());

        // Test wrong extension
        let temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        let actual = fixture.validate_function_file(temp_file.path()).await;
        assert!(actual.is_err());
    }
}
