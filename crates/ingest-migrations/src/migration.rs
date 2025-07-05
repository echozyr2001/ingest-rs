use crate::error::{MigrationError, Result};
use chrono::{DateTime, Utc};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Migration direction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationDirection {
    /// Apply migration (forward)
    Up,
    /// Rollback migration (backward)
    Down,
}

/// Migration definition
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct Migration {
    /// Unique migration identifier
    pub id: Uuid,
    /// Migration version (timestamp or sequential number)
    pub version: i64,
    /// Migration name
    pub name: String,
    /// Migration description
    pub description: Option<String>,
    /// SQL to apply the migration
    pub up_sql: String,
    /// SQL to rollback the migration
    pub down_sql: String,
    /// Migration dependencies (versions that must be applied first)
    pub dependencies: Vec<i64>,
    /// Migration metadata
    pub metadata: HashMap<String, String>,
    /// When the migration was created
    pub created_at: DateTime<Utc>,
}

impl Migration {
    /// Create a new migration
    pub fn new(
        version: i64,
        name: impl Into<String>,
        up_sql: impl Into<String>,
        down_sql: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            version,
            name: name.into(),
            description: None,
            up_sql: up_sql.into(),
            down_sql: down_sql.into(),
            dependencies: Vec::new(),
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a dependency
    pub fn with_dependency(mut self, version: i64) -> Self {
        self.dependencies.push(version);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the SQL for a specific direction
    pub fn get_sql(&self, direction: MigrationDirection) -> &str {
        match direction {
            MigrationDirection::Up => &self.up_sql,
            MigrationDirection::Down => &self.down_sql,
        }
    }

    /// Validate the migration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(MigrationError::invalid("Migration name cannot be empty"));
        }

        if self.up_sql.trim().is_empty() {
            return Err(MigrationError::invalid("Up SQL cannot be empty"));
        }

        if self.down_sql.trim().is_empty() {
            return Err(MigrationError::invalid("Down SQL cannot be empty"));
        }

        if self.version <= 0 {
            return Err(MigrationError::invalid(
                "Migration version must be positive",
            ));
        }

        Ok(())
    }

    /// Check if this migration has a specific dependency
    pub fn has_dependency(&self, version: i64) -> bool {
        self.dependencies.contains(&version)
    }

    /// Get migration filename
    pub fn filename(&self) -> String {
        format!(
            "{:04}_{}.sql",
            self.version,
            self.name.replace(' ', "_").to_lowercase()
        )
    }
}

/// Migration status in the database
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct MigrationStatus {
    /// Migration version
    pub version: i64,
    /// Migration name
    pub name: String,
    /// Whether the migration is applied
    #[setters(skip)]
    pub applied: bool,
    /// When the migration was applied (if applied)
    pub applied_at: Option<DateTime<Utc>>,
    /// Checksum of the migration SQL
    pub checksum: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: Option<i64>,
}

impl MigrationStatus {
    /// Create a new migration status
    pub fn new(version: i64, name: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            applied: false,
            applied_at: None,
            checksum: None,
            execution_time_ms: None,
        }
    }

    /// Mark as applied
    pub fn applied(mut self, execution_time_ms: i64, checksum: impl Into<String>) -> Self {
        self.applied = true;
        self.applied_at = Some(Utc::now());
        self.checksum = Some(checksum.into());
        self.execution_time_ms = Some(execution_time_ms);
        self
    }

    /// Check if the migration is pending
    pub fn is_pending(&self) -> bool {
        !self.applied
    }
}

/// Built-in migrations for the Inngest system
pub struct BuiltinMigrations;

impl BuiltinMigrations {
    /// Get all built-in migrations
    pub fn all() -> Vec<Migration> {
        vec![
            Self::create_schema_migrations_table(),
            Self::create_functions_table(),
            Self::create_function_runs_table(),
            Self::create_events_table(),
            Self::create_function_versions_table(),
        ]
    }

    /// Create the schema migrations tracking table
    fn create_schema_migrations_table() -> Migration {
        Migration::new(
            20240101000001,
            "create_schema_migrations_table",
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                applied_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                checksum VARCHAR(64),
                execution_time_ms BIGINT
            );
            
            CREATE INDEX IF NOT EXISTS idx_schema_migrations_applied_at 
            ON schema_migrations(applied_at);
            "#,
            r#"
            DROP TABLE IF EXISTS schema_migrations;
            "#,
        )
        .description("Create the schema migrations tracking table")
        .with_metadata("category", "system")
    }

    /// Create the functions table
    fn create_functions_table() -> Migration {
        Migration::new(
            20240101000002,
            "create_functions_table",
            r#"
            CREATE TABLE IF NOT EXISTS functions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name VARCHAR(255) NOT NULL,
                slug VARCHAR(255) NOT NULL UNIQUE,
                app_id UUID NOT NULL,
                config JSONB NOT NULL DEFAULT '{}',
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_functions_app_id ON functions(app_id);
            CREATE INDEX IF NOT EXISTS idx_functions_slug ON functions(slug);
            CREATE INDEX IF NOT EXISTS idx_functions_name ON functions(name);
            "#,
            r#"
            DROP TABLE IF EXISTS functions;
            "#,
        )
        .description("Create the functions table for storing function definitions")
        .with_metadata("category", "core")
        .with_dependency(20240101000001)
    }

    /// Create the function runs table
    fn create_function_runs_table() -> Migration {
        Migration::new(
            20240101000003,
            "create_function_runs_table",
            r#"
            CREATE TABLE IF NOT EXISTS function_runs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                function_id UUID NOT NULL REFERENCES functions(id),
                status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
                started_at TIMESTAMP WITH TIME ZONE,
                ended_at TIMESTAMP WITH TIME ZONE,
                output JSONB,
                error_message TEXT,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_function_runs_function_id ON function_runs(function_id);
            CREATE INDEX IF NOT EXISTS idx_function_runs_status ON function_runs(status);
            CREATE INDEX IF NOT EXISTS idx_function_runs_created_at ON function_runs(created_at);
            "#,
            r#"
            DROP TABLE IF EXISTS function_runs;
            "#,
        )
        .description("Create the function runs table for tracking function executions")
        .with_metadata("category", "core")
        .with_dependency(20240101000002)
    }

    /// Create the events table
    fn create_events_table() -> Migration {
        Migration::new(
            20240101000004,
            "create_events_table",
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name VARCHAR(255) NOT NULL,
                data JSONB NOT NULL DEFAULT '{}',
                user_id UUID,
                timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_events_name ON events(name);
            CREATE INDEX IF NOT EXISTS idx_events_user_id ON events(user_id);
            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at);
            "#,
            r#"
            DROP TABLE IF EXISTS events;
            "#,
        )
        .description("Create the events table for storing incoming events")
        .with_metadata("category", "core")
        .with_dependency(20240101000001)
    }

    /// Create the function versions table
    fn create_function_versions_table() -> Migration {
        Migration::new(
            20240101000005,
            "create_function_versions_table",
            r#"
            CREATE TABLE IF NOT EXISTS function_versions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                function_id UUID NOT NULL REFERENCES functions(id),
                version INTEGER NOT NULL,
                config JSONB NOT NULL DEFAULT '{}',
                is_active BOOLEAN NOT NULL DEFAULT false,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_function_versions_function_id ON function_versions(function_id);
            CREATE INDEX IF NOT EXISTS idx_function_versions_version ON function_versions(version);
            CREATE INDEX IF NOT EXISTS idx_function_versions_is_active ON function_versions(is_active);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_function_versions_function_version 
            ON function_versions(function_id, version);
            "#,
            r#"
            DROP TABLE IF EXISTS function_versions;
            "#,
        )
        .description("Create the function versions table for function versioning")
        .with_metadata("category", "core")
        .with_dependency(20240101000002)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_migration_creation() {
        let migration = Migration::new(
            1,
            "test_migration",
            "CREATE TABLE test (id INT);",
            "DROP TABLE test;",
        );

        assert_eq!(migration.version, 1);
        assert_eq!(migration.name, "test_migration");
        assert!(!migration.up_sql.is_empty());
        assert!(!migration.down_sql.is_empty());
    }

    #[test]
    fn test_migration_with_description() {
        let migration = Migration::new(1, "test", "CREATE TABLE test;", "DROP TABLE test;")
            .description("Test migration");

        assert_eq!(migration.description, Some("Test migration".to_string()));
    }

    #[test]
    fn test_migration_with_dependency() {
        let migration =
            Migration::new(2, "test", "CREATE TABLE test;", "DROP TABLE test;").with_dependency(1);

        assert!(migration.has_dependency(1));
        assert!(!migration.has_dependency(2));
    }

    #[test]
    fn test_migration_validation() {
        let valid_migration = Migration::new(1, "test", "CREATE TABLE test;", "DROP TABLE test;");
        assert!(valid_migration.validate().is_ok());

        let invalid_migration = Migration::new(0, "", "", "");
        assert!(invalid_migration.validate().is_err());
    }

    #[test]
    fn test_migration_direction() {
        let migration = Migration::new(1, "test", "CREATE TABLE test;", "DROP TABLE test;");

        assert_eq!(
            migration.get_sql(MigrationDirection::Up),
            "CREATE TABLE test;"
        );
        assert_eq!(
            migration.get_sql(MigrationDirection::Down),
            "DROP TABLE test;"
        );
    }

    #[test]
    fn test_migration_status() {
        let status = MigrationStatus::new(1, "test_migration");
        assert!(!status.applied);
        assert!(status.is_pending());

        let applied_status = status.applied(100, "checksum123");
        assert!(applied_status.applied);
        assert!(!applied_status.is_pending());
        assert_eq!(applied_status.execution_time_ms, Some(100));
    }

    #[test]
    fn test_migration_filename() {
        let migration = Migration::new(
            1,
            "Create Users Table",
            "CREATE TABLE users;",
            "DROP TABLE users;",
        );
        assert_eq!(migration.filename(), "0001_create_users_table.sql");
    }

    #[test]
    fn test_builtin_migrations() {
        let migrations = BuiltinMigrations::all();
        assert!(!migrations.is_empty());

        // Check that all migrations are valid
        for migration in &migrations {
            assert!(migration.validate().is_ok());
        }

        // Check that migrations are ordered by version
        for i in 1..migrations.len() {
            assert!(migrations[i - 1].version < migrations[i].version);
        }
    }

    #[test]
    fn test_migration_setters() {
        use pretty_assertions::assert_eq;

        // Test Migration setters
        let fixture = Migration::new(1, "test", "CREATE TABLE test;", "DROP TABLE test;")
            .description("Test migration description")
            .dependencies(vec![0])
            .metadata({
                let mut meta = std::collections::HashMap::new();
                meta.insert("author".to_string(), "test_user".to_string());
                meta
            });

        let actual = (
            &fixture.description,
            &fixture.dependencies,
            fixture.metadata.get("author"),
        );
        let test_user = "test_user".to_string();
        let expected = (
            &Some("Test migration description".to_string()),
            &vec![0],
            Some(&test_user),
        );

        assert_eq!(actual, expected);

        // Test MigrationStatus setters
        let status_fixture = MigrationStatus {
            version: 1,
            name: "test".to_string(),
            applied: false,
            applied_at: None,
            checksum: None,
            execution_time_ms: None,
        }
        .name("updated_migration")
        .version(2)
        .checksum("abc123")
        .execution_time_ms(150);

        let actual_status = (
            status_fixture.version,
            &status_fixture.name,
            &status_fixture.checksum,
            status_fixture.execution_time_ms,
        );
        let expected_status = (
            2,
            &"updated_migration".to_string(),
            &Some("abc123".to_string()),
            Some(150),
        );

        assert_eq!(actual_status, expected_status);
    }
}
