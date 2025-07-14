use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub inngest: InngestConfig,
    pub dev: Option<DevConfig>,
    pub deploy: Option<DeployConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub functions_dir: PathBuf,
    pub build_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InngestConfig {
    pub app_id: String,
    pub event_key: Option<String>,
    pub signing_key: Option<String>,
    pub serve_origin: Option<String>,
    pub serve_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevConfig {
    pub port: u16,
    pub host: String,
    pub auto_reload: bool,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub environment: String,
    pub url: Option<String>,
    pub env: HashMap<String, String>,
}

impl ProjectConfig {
    pub async fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!("Configuration file not found: {}", path.display());
        }

        let content = fs::read_to_string(path).await?;
        let config: ProjectConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse configuration: {}", e))?;

        Ok(config)
    }

    #[allow(dead_code)]
    pub async fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize configuration: {}", e))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(path, content).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn validate(&self) -> Result<()> {
        if self.project.name.is_empty() {
            anyhow::bail!("Project name cannot be empty");
        }

        if self.inngest.app_id.is_empty() {
            anyhow::bail!("Inngest app_id cannot be empty");
        }

        if !self.project.functions_dir.is_relative() {
            anyhow::bail!("Functions directory must be relative to project root");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_dev_config(&self) -> DevConfig {
        self.dev.clone().unwrap_or_else(|| DevConfig {
            port: 3000,
            host: "localhost".to_string(),
            auto_reload: true,
            env: HashMap::new(),
        })
    }

    #[allow(dead_code)]
    pub fn get_deploy_config(&self) -> DeployConfig {
        self.deploy.clone().unwrap_or_else(|| DeployConfig {
            environment: "development".to_string(),
            url: None,
            env: HashMap::new(),
        })
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            project: ProjectInfo {
                name: "my-inngest-app".to_string(),
                version: "0.1.0".to_string(),
                functions_dir: PathBuf::from("src/functions"),
                build_dir: PathBuf::from("target/inngest"),
            },
            inngest: InngestConfig {
                app_id: "my-app".to_string(),
                event_key: None,
                signing_key: None,
                serve_origin: None,
                serve_path: "/api/inngest".to_string(),
            },
            dev: Some(DevConfig {
                port: 3000,
                host: "localhost".to_string(),
                auto_reload: true,
                env: HashMap::new(),
            }),
            deploy: Some(DeployConfig {
                environment: "development".to_string(),
                url: None,
                env: HashMap::new(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_config_save_and_load() {
        let fixture = ProjectConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Save config
        fixture.save(path).await.unwrap();

        // Load config
        let actual = ProjectConfig::load(path).await.unwrap();
        let expected = fixture.project.name;
        assert_eq!(actual.project.name, expected);
    }

    #[test]
    fn test_config_validation() {
        let mut fixture = ProjectConfig::default();
        fixture.project.name = "".to_string();

        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_dev_config_defaults() {
        let fixture = ProjectConfig {
            dev: None,
            ..Default::default()
        };

        let actual = fixture.get_dev_config();
        let expected = 3000;
        assert_eq!(actual.port, expected);
    }
}
