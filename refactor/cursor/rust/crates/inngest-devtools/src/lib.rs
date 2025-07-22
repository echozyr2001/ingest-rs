//! Development tools for Inngest
//!
//! This module provides development-friendly implementations of storage backends,
//! including embedded Redis-like storage and SQLite database support.

use chrono::{DateTime, Utc};
use inngest_core::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod dev_config;
pub mod embedded_redis;
pub mod sqlite_support;

pub use dev_config::DevConfig;
pub use embedded_redis::EmbeddedRedis;
pub use sqlite_support::{SqliteDatabase, SqliteStateManager};

/// Development environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevEnvironmentConfig {
    /// Whether to use in-memory storage only (no persistence to disk)
    pub in_memory: bool,
    /// Redis configuration
    pub redis: RedisConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Directory for data storage (when not in-memory)
    pub data_dir: Option<String>,
}

impl Default for DevEnvironmentConfig {
    fn default() -> Self {
        Self {
            in_memory: true,
            redis: RedisConfig::default(),
            database: DatabaseConfig::default(),
            data_dir: None,
        }
    }
}

/// Redis configuration for development
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// External Redis URI (if empty, use embedded Redis)
    pub external_uri: Option<String>,
    /// Whether to enable persistence for embedded Redis
    pub persist: bool,
    /// Tick interval for time simulation (in milliseconds)
    pub tick_ms: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            external_uri: None,
            persist: false,
            tick_ms: 150,
        }
    }
}

/// Database configuration for development
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// External PostgreSQL URI (if empty, use SQLite)
    pub external_uri: Option<String>,
    /// SQLite file path (if None and in_memory is false, use default path)
    pub sqlite_path: Option<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            external_uri: None,
            sqlite_path: None,
        }
    }
}

/// Development environment manager
pub struct DevEnvironment {
    config: DevEnvironmentConfig,
    embedded_redis: Option<Arc<embedded_redis::EmbeddedRedis>>,
    sqlite_db: Option<Arc<sqlite_support::SqliteDatabase>>,
}

impl DevEnvironment {
    /// Create a new development environment
    pub async fn new(config: DevEnvironmentConfig) -> Result<Self> {
        let embedded_redis = if config.in_memory {
            let redis = embedded_redis::EmbeddedRedis::new(embedded_redis::EmbeddedRedisConfig {
                tick_interval_ms: config.redis.tick_ms,
                data_dir: config.data_dir.clone(),
                persist: false,
            })
            .await?;
            Some(Arc::new(redis))
        } else {
            None
        };

        let sqlite_db = if !config.in_memory {
            if config.database.external_uri.is_none() {
                let path = config
                    .database
                    .sqlite_path
                    .clone()
                    .or_else(|| {
                        config
                            .data_dir
                            .as_ref()
                            .map(|d| format!("{}/inngest.db", d))
                    })
                    .unwrap_or_else(|| "./inngest-dev.db".to_string());
                Some(Arc::new(
                    sqlite_support::SqliteDatabase::new_file(&path).await?,
                ))
            } else {
                // No external DB support for now
                None
            }
        } else {
            // When in_memory is true, we still want to create an in-memory sqlite db
            Some(Arc::new(
                sqlite_support::SqliteDatabase::new_memory().await?,
            ))
        };

        Ok(Self {
            config,
            embedded_redis,
            sqlite_db,
        })
    }

    /// Get the state manager
    pub fn state_manager(&self) -> Arc<dyn inngest_state::StateManager> {
        // For now, return an in-memory manager
        Arc::new(inngest_state::InMemoryStateManager::new())
    }

    /// Get the queue manager
    pub fn queue_manager(&self) -> Arc<dyn inngest_queue::QueueManager> {
        // For now, return an in-memory manager
        Arc::new(inngest_queue::InMemoryQueueManager::new())
    }

    /// Get the embedded Redis instance (if available)
    pub fn embedded_redis(&self) -> Option<Arc<embedded_redis::EmbeddedRedis>> {
        self.embedded_redis.clone()
    }

    /// Get the SQLite database instance (if available)
    pub fn sqlite_database(&self) -> Option<Arc<sqlite_support::SqliteDatabase>> {
        self.sqlite_db.clone()
    }

    /// Reset all data (useful for testing)
    pub async fn reset_all(&self) -> Result<()> {
        if let Some(redis) = &self.embedded_redis {
            redis.flush_all().await?;
        }

        if let Some(db) = &self.sqlite_db {
            db.truncate_all().await?;
        }

        Ok(())
    }

    /// Export snapshot of all data
    pub async fn export_snapshot(&self) -> Result<DevSnapshot> {
        let mut redis_data = None;
        let mut sqlite_data = None;

        if let Some(redis) = &self.embedded_redis {
            redis_data = Some(redis.export_data().await?);
        }

        if let Some(db) = &self.sqlite_db {
            sqlite_data = Some(db.export_data().await?);
        }

        Ok(DevSnapshot {
            timestamp: Utc::now(),
            redis_data,
            sqlite_data,
        })
    }

    /// Import snapshot data
    pub async fn import_snapshot(&self, snapshot: DevSnapshot) -> Result<()> {
        if let (Some(redis), Some(data)) = (&self.embedded_redis, &snapshot.redis_data) {
            redis.import_data(data.clone()).await?;
        }

        if let (Some(db), Some(data)) = (&self.sqlite_db, &snapshot.sqlite_data) {
            db.import_data(data.clone()).await?;
        }

        Ok(())
    }
}

/// Development environment data snapshot
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevSnapshot {
    pub timestamp: DateTime<Utc>,
    pub redis_data: Option<embedded_redis::RedisSnapshot>,
    pub sqlite_data: Option<sqlite_support::SqliteSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_dev_environment_creation() {
        let config = DevEnvironmentConfig::default();
        let env = DevEnvironment::new(config).await.unwrap();

        // Verify components are created
        assert!(env.sqlite_db.is_some());
        assert!(env.embedded_redis.is_some());
    }

    #[tokio::test]
    async fn test_dev_environment_reset() {
        let config = DevEnvironmentConfig::default();
        let env = DevEnvironment::new(config).await.unwrap();

        env.reset_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_snapshot_export_import() {
        let config = DevEnvironmentConfig::default();
        let env = DevEnvironment::new(config).await.unwrap();

        // TODO: Add some data to the environment to make the test more meaningful

        let exported = env.export_snapshot().await.unwrap();
        env.reset_all().await.unwrap();
        env.import_snapshot(exported.clone()).await.unwrap();
        let imported = env.export_snapshot().await.unwrap();

        // The timestamp will be different, so we can't compare them directly.
        // We'll compare the data parts.
        assert_eq!(exported.redis_data, imported.redis_data);
        assert_eq!(exported.sqlite_data, imported.sqlite_data);
    }
}
