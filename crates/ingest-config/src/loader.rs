use crate::{Config, ConfigError, Result};
use config::{ConfigBuilder, Environment, File};
use std::path::Path;

/// Configuration loader with support for multiple sources
pub struct ConfigLoader {
    builder: ConfigBuilder<config::builder::DefaultState>,
    env_prefix: String,
    files: Vec<String>,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            builder: config::Config::builder(),
            env_prefix: "INNGEST".to_string(),
            files: Vec::new(),
        }
    }

    /// Add a configuration file
    pub fn with_file(mut self, path: &str) -> Self {
        self.files.push(path.to_string());
        self
    }

    /// Set environment variable prefix
    pub fn with_env_prefix(mut self, prefix: &str) -> Self {
        self.env_prefix = prefix.to_string();
        self
    }

    /// Load configuration from all sources
    pub fn load(mut self) -> Result<Config> {
        // Add default configuration
        self.builder = self.builder.set_default("database.host", "localhost")?;
        self.builder = self.builder.set_default("database.port", 5432)?;
        self.builder = self.builder.set_default("database.database", "inngest")?;
        self.builder = self.builder.set_default("database.username", "postgres")?;
        self.builder = self.builder.set_default("database.password", "postgres")?;
        self.builder = self
            .builder
            .set_default("redis.url", "redis://localhost:6379")?;
        self.builder = self.builder.set_default("server.host", "0.0.0.0")?;
        self.builder = self.builder.set_default("server.port", 8080)?;
        self.builder = self.builder.set_default("logging.level", "info")?;
        self.builder = self.builder.set_default("logging.format", "json")?;

        // Add configuration files
        for file_path in &self.files {
            if Path::new(file_path).exists() {
                self.builder = self.builder.add_source(File::with_name(file_path));
            } else {
                return Err(ConfigError::file(format!(
                    "Configuration file not found: {}",
                    file_path
                )));
            }
        }

        // Add environment variables
        self.builder = self.builder.add_source(
            Environment::with_prefix(&self.env_prefix)
                .separator("_")
                .try_parsing(true),
        );

        // Build and deserialize configuration
        let config = self.builder.build()?;
        let app_config: Config = config.try_deserialize()?;

        // Validate configuration
        app_config.validate()?;

        Ok(app_config)
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::env;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_loader_new() {
        let actual = ConfigLoader::new();
        assert_eq!(actual.env_prefix, "INNGEST");
        assert!(actual.files.is_empty());
    }

    #[test]
    fn test_config_loader_with_file() {
        let fixture = ConfigLoader::new().with_file("config.toml");
        assert_eq!(fixture.files, vec!["config.toml"]);
    }

    #[test]
    fn test_config_loader_with_env_prefix() {
        let fixture = ConfigLoader::new().with_env_prefix("TEST");
        assert_eq!(fixture.env_prefix, "TEST");
    }

    #[test]
    fn test_config_loader_load_defaults() {
        let actual = ConfigLoader::new().load();
        assert!(actual.is_ok());

        let config = actual.unwrap();
        assert_eq!(config.database.host, "localhost");
        assert_eq!(config.database.port, 5432);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_config_loader_load_with_env() {
        unsafe {
            env::set_var("INNGEST_DATABASE_HOST", "db.example.com");
            env::set_var("INNGEST_SERVER_PORT", "9090");
        }

        let actual = ConfigLoader::new().load();
        assert!(actual.is_ok());

        let config = actual.unwrap();
        assert_eq!(config.database.host, "db.example.com");
        assert_eq!(config.server.port, 9090);

        // Clean up
        unsafe {
            env::remove_var("INNGEST_DATABASE_HOST");
            env::remove_var("INNGEST_SERVER_PORT");
        }
    }

    #[test]
    fn test_config_loader_load_with_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
[database]
host = "file.example.com"
port = 5433

[server]
host = "127.0.0.1"
port = 3000
        "#
        )
        .unwrap();

        let actual = ConfigLoader::new()
            .with_file(temp_file.path().to_str().unwrap())
            .load();
        assert!(actual.is_ok());

        let config = actual.unwrap();
        assert_eq!(config.database.host, "file.example.com");
        assert_eq!(config.database.port, 5433);
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
    }

    #[test]
    fn test_config_loader_load_file_not_found() {
        let actual = ConfigLoader::new().with_file("nonexistent.toml").load();
        assert!(actual.is_err());
    }
}
