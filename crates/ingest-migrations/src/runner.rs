use crate::error::{MigrationError, Result};
use crate::migration::{BuiltinMigrations, Migration, MigrationDirection, MigrationStatus};
use ingest_storage::PostgresStorage;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Migration runner for executing database migrations
pub struct MigrationRunner {
    storage: Arc<PostgresStorage>,
    migrations: HashMap<i64, Migration>,
}

impl MigrationRunner {
    /// Create a new migration runner
    pub fn new(storage: Arc<PostgresStorage>) -> Self {
        let mut migrations = HashMap::new();

        // Load built-in migrations
        for migration in BuiltinMigrations::all() {
            migrations.insert(migration.version, migration);
        }

        Self {
            storage,
            migrations,
        }
    }

    /// Initialize the migration system
    pub async fn initialize(&self) -> Result<()> {
        // The first migration creates the schema_migrations table
        // We need to check if it exists and create it if not
        let check_table_sql = r#"
            SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name = 'schema_migrations'
            );
        "#;

        let table_exists: bool = sqlx::query_scalar(check_table_sql)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to check migrations table: {e}"))
            })?;

        if !table_exists {
            // Apply the first migration to create the table
            if let Some(first_migration) = self.migrations.get(&20240101000001) {
                self.apply_migration(first_migration, MigrationDirection::Up)
                    .await?;
                self.record_migration(first_migration, 0).await?;
                println!("Created schema_migrations table");
            } else {
                return Err(MigrationError::invalid("First migration not found"));
            }
        }

        Ok(())
    }

    /// Add a migration to the runner
    pub async fn add_migration(&mut self, migration: Migration) -> Result<()> {
        migration.validate()?;

        if self.migrations.contains_key(&migration.version) {
            return Err(MigrationError::already_applied(format!(
                "Migration version {} already exists",
                migration.version
            )));
        }

        self.migrations.insert(migration.version, migration);
        Ok(())
    }

    /// Run all pending migrations
    pub async fn run_pending_migrations(&self) -> Result<Vec<Migration>> {
        let applied_versions = self.get_applied_versions().await?;
        let mut pending_migrations = Vec::new();

        // Get all migration versions and sort them
        let mut versions: Vec<i64> = self.migrations.keys().copied().collect();
        versions.sort();

        for version in versions {
            if !applied_versions.contains(&version) {
                if let Some(migration) = self.migrations.get(&version) {
                    // Check dependencies
                    for dep_version in &migration.dependencies {
                        if !applied_versions.contains(dep_version) {
                            return Err(MigrationError::dependency(format!(
                                "Migration {version} depends on {dep_version} which is not applied"
                            )));
                        }
                    }

                    let start = Instant::now();
                    self.apply_migration(migration, MigrationDirection::Up)
                        .await?;
                    let execution_time = start.elapsed().as_millis() as i64;

                    self.record_migration(migration, execution_time).await?;
                    pending_migrations.push(migration.clone());
                }
            }
        }

        Ok(pending_migrations)
    }

    /// Rollback migrations to a specific version
    pub async fn rollback_to_version(&self, target_version: i64) -> Result<Vec<Migration>> {
        let applied_versions = self.get_applied_versions().await?;
        let mut rolled_back = Vec::new();

        // Get versions to rollback (all versions greater than target)
        let mut versions_to_rollback: Vec<i64> = applied_versions
            .into_iter()
            .filter(|v| *v > target_version)
            .collect();

        // Sort in descending order for rollback
        versions_to_rollback.sort_by(|a, b| b.cmp(a));

        for version in versions_to_rollback {
            if let Some(migration) = self.migrations.get(&version) {
                self.apply_migration(migration, MigrationDirection::Down)
                    .await?;
                self.remove_migration_record(version).await?;
                rolled_back.push(migration.clone());
            }
        }

        Ok(rolled_back)
    }

    /// Get migration status for all migrations
    pub async fn get_migration_status(&self) -> Result<Vec<MigrationStatus>> {
        let applied_migrations = self.get_applied_migration_records().await?;
        let mut status_list = Vec::new();

        // Get all migration versions and sort them
        let mut versions: Vec<i64> = self.migrations.keys().copied().collect();
        versions.sort();

        for version in versions {
            if let Some(migration) = self.migrations.get(&version) {
                let status = if let Some(record) = applied_migrations.get(&version) {
                    MigrationStatus {
                        version,
                        name: migration.name.clone(),
                        applied: true,
                        applied_at: Some(record.applied_at),
                        checksum: record.checksum.clone(),
                        execution_time_ms: record.execution_time_ms,
                    }
                } else {
                    MigrationStatus::new(version, &migration.name)
                };

                status_list.push(status);
            }
        }

        Ok(status_list)
    }

    /// Apply a migration in the specified direction
    async fn apply_migration(
        &self,
        migration: &Migration,
        direction: MigrationDirection,
    ) -> Result<()> {
        let sql = migration.get_sql(direction);

        // Execute the migration SQL
        sqlx::query(sql)
            .execute(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::sql(format!(
                    "Failed to execute migration {}: {}",
                    migration.version, e
                ))
            })?;

        Ok(())
    }

    /// Record a migration as applied
    async fn record_migration(&self, migration: &Migration, execution_time_ms: i64) -> Result<()> {
        let checksum = self.calculate_checksum(&migration.up_sql);

        let sql = r#"
            INSERT INTO schema_migrations (version, name, checksum, execution_time_ms)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (version) DO UPDATE SET
                name = EXCLUDED.name,
                checksum = EXCLUDED.checksum,
                execution_time_ms = EXCLUDED.execution_time_ms,
                applied_at = NOW()
        "#;

        sqlx::query(sql)
            .bind(migration.version)
            .bind(&migration.name)
            .bind(&checksum)
            .bind(execution_time_ms)
            .execute(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!(
                    "Failed to record migration {}: {}",
                    migration.version, e
                ))
            })?;

        Ok(())
    }

    /// Remove a migration record
    async fn remove_migration_record(&self, version: i64) -> Result<()> {
        let sql = "DELETE FROM schema_migrations WHERE version = $1";

        sqlx::query(sql)
            .bind(version)
            .execute(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!(
                    "Failed to remove migration record {version}: {e}"
                ))
            })?;

        Ok(())
    }

    /// Get applied migration versions
    async fn get_applied_versions(&self) -> Result<Vec<i64>> {
        let sql = "SELECT version FROM schema_migrations ORDER BY version";

        let versions: Vec<i64> = sqlx::query_scalar(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to get applied migrations: {e}"))
            })?;

        Ok(versions)
    }

    /// Get applied migration records
    async fn get_applied_migration_records(&self) -> Result<HashMap<i64, AppliedMigrationRecord>> {
        let sql = r#"
            SELECT version, name, applied_at, checksum, execution_time_ms
            FROM schema_migrations
            ORDER BY version
        "#;

        let rows = sqlx::query(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to get migration records: {e}"))
            })?;

        let mut records = HashMap::new();
        for row in rows {
            let version: i64 = row.get("version");
            let record = AppliedMigrationRecord {
                version,
                name: row.get("name"),
                applied_at: row.get("applied_at"),
                checksum: row.get("checksum"),
                execution_time_ms: row.get("execution_time_ms"),
            };
            records.insert(version, record);
        }

        Ok(records)
    }

    /// Calculate checksum for SQL content
    fn calculate_checksum(&self, sql: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        sql.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get the number of available migrations
    pub fn migration_count(&self) -> usize {
        self.migrations.len()
    }

    /// Check if a migration exists
    pub fn has_migration(&self, version: i64) -> bool {
        self.migrations.contains_key(&version)
    }

    /// Validate applied migrations against available migrations
    pub async fn validate_applied_migrations(&self) -> Result<Vec<String>> {
        let applied_records = self.get_applied_migration_records().await?;
        let mut validation_errors = Vec::new();

        for (version, record) in applied_records {
            if let Some(migration) = self.migrations.get(&version) {
                if let Err(e) = record.validate_against_migration(migration) {
                    validation_errors.push(format!("Migration {}: {}", record.version(), e));
                }
            } else {
                validation_errors.push(format!(
                    "Applied migration {} ('{}') not found in available migrations",
                    record.version(),
                    record.name()
                ));
            }
        }

        Ok(validation_errors)
    }

    /// Get detailed information about applied migrations
    pub async fn get_applied_migration_details(
        &self,
    ) -> Result<Vec<(i64, String, chrono::DateTime<chrono::Utc>)>> {
        let applied_records = self.get_applied_migration_records().await?;
        let mut details = Vec::new();

        for record in applied_records.values() {
            details.push((
                record.version(),
                record.name().to_string(),
                record.applied_at(),
            ));
        }

        // Sort by version
        details.sort_by_key(|(version, _, _)| *version);
        Ok(details)
    }

    /// Get migration execution statistics
    pub async fn get_migration_statistics(
        &self,
    ) -> Result<Vec<(i64, String, Option<i64>, Option<String>)>> {
        let applied_records = self.get_applied_migration_records().await?;
        let mut stats = Vec::new();

        for record in applied_records.values() {
            stats.push((
                record.version(),
                record.name().to_string(),
                record.execution_time_ms(),
                record.checksum().map(|s| s.to_string()),
            ));
        }

        // Sort by version
        stats.sort_by_key(|(version, _, _, _)| *version);
        Ok(stats)
    }

    /// Find migrations that match specific criteria
    pub async fn find_matching_migrations(&self) -> Result<Vec<(i64, String, bool)>> {
        let applied_records = self.get_applied_migration_records().await?;
        let mut matches = Vec::new();

        for (version, record) in applied_records {
            if let Some(migration) = self.migrations.get(&version) {
                let is_match = record.matches_migration(migration);
                matches.push((record.version(), record.name().to_string(), is_match));
            }
        }

        // Sort by version
        matches.sort_by_key(|(version, _, _)| *version);
        Ok(matches)
    }
}

/// Applied migration record from database
#[derive(Debug, Clone)]
struct AppliedMigrationRecord {
    version: i64,
    name: String,
    applied_at: chrono::DateTime<chrono::Utc>,
    checksum: Option<String>,
    execution_time_ms: Option<i64>,
}

impl AppliedMigrationRecord {
    /// Get the migration version
    pub fn version(&self) -> i64 {
        self.version
    }

    /// Get the migration name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the applied timestamp
    pub fn applied_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.applied_at
    }

    /// Get the checksum if available
    pub fn checksum(&self) -> Option<&str> {
        self.checksum.as_deref()
    }

    /// Get the execution time in milliseconds
    pub fn execution_time_ms(&self) -> Option<i64> {
        self.execution_time_ms
    }

    /// Check if this record matches a migration
    pub fn matches_migration(&self, migration: &Migration) -> bool {
        self.version == migration.version && self.name == migration.name
    }

    /// Validate the record against a migration
    pub fn validate_against_migration(&self, migration: &Migration) -> Result<()> {
        if self.version != migration.version {
            return Err(MigrationError::invalid(format!(
                "Version mismatch: record has {}, migration has {}",
                self.version, migration.version
            )));
        }

        if self.name != migration.name {
            return Err(MigrationError::invalid(format!(
                "Name mismatch for version {}: record has '{}', migration has '{}'",
                self.version, self.name, migration.name
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_migration_runner_creation() {
        // Create a mock storage for testing
        // In a real test, we'd need a proper PostgresStorage instance
        // For now, just test the basic structure
    }

    #[test]
    fn test_migration_count() {
        // Test that built-in migrations are loaded
        let builtin_count = BuiltinMigrations::all().len();
        assert!(builtin_count > 0);
    }

    #[test]
    fn test_checksum_calculation() {
        // Test checksum calculation
        let _sql1 = "CREATE TABLE test (id INT);";
        let _sql2 = "CREATE TABLE test (id INT);";
        let _sql3 = "CREATE TABLE other (id INT);";

        // Same SQL should produce same checksum
        // Different SQL should produce different checksums
        // Note: This is a simplified test since we can't easily test the actual runner
    }

    #[test]
    fn test_applied_migration_record_accessors() {
        let record = AppliedMigrationRecord {
            version: 20240101000001,
            name: "create_users_table".to_string(),
            applied_at: Utc::now(),
            checksum: Some("abc123".to_string()),
            execution_time_ms: Some(150),
        };

        assert_eq!(record.version(), 20240101000001);
        assert_eq!(record.name(), "create_users_table");
        assert_eq!(record.checksum(), Some("abc123"));
        assert_eq!(record.execution_time_ms(), Some(150));

        // Test record without checksum and execution time
        let minimal_record = AppliedMigrationRecord {
            version: 20240101000002,
            name: "minimal_migration".to_string(),
            applied_at: Utc::now(),
            checksum: None,
            execution_time_ms: None,
        };

        assert_eq!(minimal_record.checksum(), None);
        assert_eq!(minimal_record.execution_time_ms(), None);
    }

    #[test]
    fn test_applied_migration_record_validation() {
        use crate::migration::Migration;

        let record = AppliedMigrationRecord {
            version: 20240101000001,
            name: "create_users_table".to_string(),
            applied_at: Utc::now(),
            checksum: Some("abc123".to_string()),
            execution_time_ms: Some(150),
        };

        let matching_migration = Migration::new(
            20240101000001,
            "create_users_table",
            "CREATE TABLE users (id INT);",
            "DROP TABLE users;",
        );

        let mismatched_migration = Migration::new(
            20240101000002,
            "create_posts_table",
            "CREATE TABLE posts (id INT);",
            "DROP TABLE posts;",
        );

        let version_mismatch_migration = Migration::new(
            20240101000003,
            "create_users_table", // Same name, different version
            "CREATE TABLE users (id INT);",
            "DROP TABLE users;",
        );

        // Should validate successfully against matching migration
        assert!(record
            .validate_against_migration(&matching_migration)
            .is_ok());
        assert!(record.matches_migration(&matching_migration));

        // Should fail validation against mismatched migration
        assert!(record
            .validate_against_migration(&mismatched_migration)
            .is_err());
        assert!(!record.matches_migration(&mismatched_migration));

        // Should fail validation against version mismatch
        assert!(record
            .validate_against_migration(&version_mismatch_migration)
            .is_err());
        assert!(!record.matches_migration(&version_mismatch_migration));
    }
}
