use crate::{ConfigError, Result};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};

/// Database configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct DatabaseConfig {
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub database: String,
    /// Database username
    pub username: String,
    /// Database password
    pub password: String,
    /// Maximum number of connections in pool
    pub max_connections: Option<u32>,
    /// Minimum number of connections in pool
    pub min_connections: Option<u32>,
    /// Connection timeout
    pub connect_timeout: Option<std::time::Duration>,
    /// Command timeout
    pub command_timeout: Option<std::time::Duration>,
    /// Enable SSL
    pub ssl: Option<bool>,
    /// SSL mode (require, prefer, disable)
    pub ssl_mode: Option<String>,
    /// Connection pool idle timeout
    pub idle_timeout: Option<std::time::Duration>,
    /// Maximum connection lifetime
    pub max_lifetime: Option<std::time::Duration>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "inngest".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            max_connections: Some(10),
            min_connections: Some(1),
            connect_timeout: Some(std::time::Duration::from_secs(30)),
            command_timeout: Some(std::time::Duration::from_secs(60)),
            ssl: Some(false),
            ssl_mode: Some("prefer".to_string()),
            idle_timeout: Some(std::time::Duration::from_secs(600)), // 10 minutes
            max_lifetime: Some(std::time::Duration::from_secs(3600)), // 1 hour
        }
    }
}

impl DatabaseConfig {
    /// Get the database connection URL
    pub fn url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }

    /// Get the database connection URL with SSL mode
    pub fn url_with_ssl(&self) -> String {
        let mut url = self.url();

        if let Some(ssl_mode) = &self.ssl_mode {
            url.push_str(&format!("?sslmode={ssl_mode}"));
        }

        url
    }

    /// Validate the database configuration
    pub fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(ConfigError::validation("Database host cannot be empty"));
        }

        if self.port == 0 {
            return Err(ConfigError::validation("Database port must be > 0"));
        }

        if self.database.is_empty() {
            return Err(ConfigError::validation("Database name cannot be empty"));
        }

        if self.username.is_empty() {
            return Err(ConfigError::validation("Database username cannot be empty"));
        }

        if let Some(max_conn) = self.max_connections {
            if max_conn == 0 {
                return Err(ConfigError::validation(
                    "Database max_connections must be > 0",
                ));
            }
        }

        if let Some(min_conn) = self.min_connections {
            if min_conn == 0 {
                return Err(ConfigError::validation(
                    "Database min_connections must be > 0",
                ));
            }
        }

        if let (Some(max_conn), Some(min_conn)) = (self.max_connections, self.min_connections) {
            if min_conn > max_conn {
                return Err(ConfigError::validation(
                    "Database min_connections cannot be greater than max_connections",
                ));
            }
        }

        if let Some(ssl_mode) = &self.ssl_mode {
            let valid_modes = [
                "disable",
                "allow",
                "prefer",
                "require",
                "verify-ca",
                "verify-full",
            ];
            if !valid_modes.contains(&ssl_mode.as_str()) {
                return Err(ConfigError::validation(format!(
                    "Invalid SSL mode: {}. Must be one of: {}",
                    ssl_mode,
                    valid_modes.join(", ")
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_database_config_default() {
        let actual = DatabaseConfig::default();
        let expected = DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "inngest".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            max_connections: Some(10),
            min_connections: Some(1),
            connect_timeout: Some(std::time::Duration::from_secs(30)),
            command_timeout: Some(std::time::Duration::from_secs(60)),
            ssl: Some(false),
            ssl_mode: Some("prefer".to_string()),
            idle_timeout: Some(std::time::Duration::from_secs(600)),
            max_lifetime: Some(std::time::Duration::from_secs(3600)),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_database_config_url() {
        let fixture = DatabaseConfig::default();
        let actual = fixture.url();
        let expected = "postgresql://postgres:postgres@localhost:5432/inngest";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_database_config_url_with_ssl() {
        let fixture = DatabaseConfig::default();
        let actual = fixture.url_with_ssl();
        let expected = "postgresql://postgres:postgres@localhost:5432/inngest?sslmode=prefer";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_database_config_setters() {
        let fixture = DatabaseConfig::default()
            .host("db.example.com")
            .port(5433u16)
            .database("myapp")
            .username("user")
            .password("secret")
            .max_connections(20u32);

        assert_eq!(fixture.host, "db.example.com");
        assert_eq!(fixture.port, 5433);
        assert_eq!(fixture.database, "myapp");
        assert_eq!(fixture.username, "user");
        assert_eq!(fixture.password, "secret");
        assert_eq!(fixture.max_connections, Some(20));
    }

    #[test]
    fn test_database_config_validation_success() {
        let fixture = DatabaseConfig::default();
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_database_config_validation_empty_host() {
        let fixture = DatabaseConfig::default().host("");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_zero_port() {
        let fixture = DatabaseConfig::default().port(0u16);
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_empty_database() {
        let fixture = DatabaseConfig::default().database("");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_empty_username() {
        let fixture = DatabaseConfig::default().username("");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_zero_max_connections() {
        let fixture = DatabaseConfig::default().max_connections(0u32);
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_min_greater_than_max() {
        let fixture = DatabaseConfig::default()
            .min_connections(10u32)
            .max_connections(5u32);
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_validation_invalid_ssl_mode() {
        let fixture = DatabaseConfig::default().ssl_mode("invalid");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_database_config_serialization() {
        let fixture = DatabaseConfig::default();
        let actual = toml::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_database_config_deserialization() {
        let fixture = DatabaseConfig::default();
        let serialized = toml::to_string(&fixture).unwrap();
        let actual: DatabaseConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(actual.host, fixture.host);
        assert_eq!(actual.port, fixture.port);
        assert_eq!(actual.database, fixture.database);
    }
}
