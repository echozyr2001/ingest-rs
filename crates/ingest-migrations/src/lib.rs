//! # Inngest Database Migrations
//!
//! This crate provides database migration management for the Inngest platform.
//!
//! ## Features
//!
//! - Version-based migration system
//! - Forward and rollback migrations
//! - Migration status tracking
//! - SQL script execution
//! - Schema validation

pub mod error;
pub mod migration;
pub mod runner;
pub mod schema;

use crate::error::{MigrationError, Result};
use crate::migration::Migration;
use crate::runner::MigrationRunner;
use crate::schema::SchemaManager;
use ingest_config::DatabaseConfig;
use ingest_storage::PostgresStorage;
use std::sync::Arc;

pub use error::Result as MigrationResult;
pub use migration::MigrationStatus;
pub use schema::SchemaInfo;

/// Migration system for managing database schema changes
pub struct MigrationSystem {
    storage: Arc<PostgresStorage>,
    runner: MigrationRunner,
    schema_manager: SchemaManager,
}

impl MigrationSystem {
    /// Create a new migration system
    pub async fn new(storage: Arc<PostgresStorage>) -> Result<Self> {
        let runner = MigrationRunner::new(storage.clone());
        let schema_manager = SchemaManager::new(storage.clone());

        Ok(Self {
            storage,
            runner,
            schema_manager,
        })
    }

    /// Initialize the migration system
    pub async fn initialize(&self) -> Result<()> {
        println!("Initializing migration system");

        // Create migration tracking table
        self.runner.initialize().await?;

        // Initialize schema manager
        self.schema_manager.initialize().await?;

        println!("Migration system initialized successfully");
        Ok(())
    }

    /// Run all pending migrations
    pub async fn migrate(&self) -> Result<Vec<Migration>> {
        println!("Running pending migrations");
        let applied = self.runner.run_pending_migrations().await?;

        if applied.is_empty() {
            println!("No pending migrations to apply");
        } else {
            println!("Applied {} migrations", applied.len());
            for migration in &applied {
                println!("  - {}: {}", migration.version, migration.name);
            }
        }

        Ok(applied)
    }

    /// Rollback migrations to a specific version
    pub async fn rollback(&self, target_version: i64) -> Result<Vec<Migration>> {
        println!("Rolling back migrations to version {target_version}");
        let rolled_back = self.runner.rollback_to_version(target_version).await?;

        if rolled_back.is_empty() {
            println!("No migrations to rollback");
        } else {
            println!("Rolled back {} migrations", rolled_back.len());
            for migration in &rolled_back {
                println!("  - {}: {}", migration.version, migration.name);
            }
        }

        Ok(rolled_back)
    }

    /// Get migration status
    pub async fn status(&self) -> Result<Vec<migration::MigrationStatus>> {
        self.runner.get_migration_status().await
    }

    /// Get current schema information
    pub async fn schema_info(&self) -> Result<schema::SchemaInfo> {
        self.schema_manager.get_schema_info().await
    }

    /// Validate current schema
    pub async fn validate_schema(&self) -> Result<bool> {
        self.schema_manager.validate_schema().await
    }

    /// Add a new migration
    pub async fn add_migration(&mut self, migration: Migration) -> Result<()> {
        self.runner.add_migration(migration).await
    }

    /// Get the storage instance
    pub fn storage(&self) -> &PostgresStorage {
        &self.storage
    }

    /// Get the migration runner
    pub fn runner(&self) -> &MigrationRunner {
        &self.runner
    }

    /// Get the schema manager
    pub fn schema_manager(&self) -> &SchemaManager {
        &self.schema_manager
    }
}

/// Create a migration system from database config
pub async fn create_migration_system(config: &DatabaseConfig) -> Result<MigrationSystem> {
    let storage = Arc::new(
        PostgresStorage::new(config)
            .await
            .map_err(|e| MigrationError::database(format!("Failed to create storage: {e}")))?,
    );

    MigrationSystem::new(storage).await
}

/// Initialize and run migrations
pub async fn run_migrations(config: &DatabaseConfig) -> Result<()> {
    let system = create_migration_system(config).await?;
    system.initialize().await?;
    system.migrate().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_config() -> DatabaseConfig {
        DatabaseConfig::default()
    }

    #[test]
    fn test_migration_system_creation() {
        // Test that we can create the basic structures
        let config = create_test_config();
        assert_eq!(config.host, "localhost");
    }

    #[tokio::test]
    async fn test_migration_system_interface() {
        // Test the interface without requiring a real database
        // In a real test environment, this would require proper setup
    }
}
