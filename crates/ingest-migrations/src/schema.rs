use crate::error::{MigrationError, Result};
use derive_setters::Setters;
use ingest_storage::PostgresStorage;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

/// Schema manager for database schema validation and information
pub struct SchemaManager {
    storage: Arc<PostgresStorage>,
}

impl SchemaManager {
    /// Create a new schema manager
    pub fn new(storage: Arc<PostgresStorage>) -> Self {
        Self { storage }
    }

    /// Initialize the schema manager
    pub async fn initialize(&self) -> Result<()> {
        // Schema manager initialization if needed
        println!("Schema manager initialized");
        Ok(())
    }

    /// Get current schema information
    pub async fn get_schema_info(&self) -> Result<SchemaInfo> {
        let tables = self.get_tables().await?;
        let indexes = self.get_indexes().await?;
        let constraints = self.get_constraints().await?;

        Ok(SchemaInfo {
            tables,
            indexes,
            constraints,
            version: self.get_current_version().await?,
            last_migration_at: self.get_last_migration_time().await?,
        })
    }

    /// Validate the current schema
    pub async fn validate_schema(&self) -> Result<bool> {
        // Check that all expected tables exist
        let expected_tables = vec![
            "schema_migrations",
            "functions",
            "function_runs",
            "events",
            "function_versions",
        ];

        for table_name in expected_tables {
            if !self.table_exists(table_name).await? {
                println!("Warning: Expected table '{table_name}' not found");
                return Ok(false);
            }
        }

        // Check that schema_migrations table has the correct structure
        if !self.validate_migrations_table().await? {
            println!("Warning: schema_migrations table structure is invalid");
            return Ok(false);
        }

        println!("Schema validation passed");
        Ok(true)
    }

    /// Check if a table exists
    pub async fn table_exists(&self, table_name: &str) -> Result<bool> {
        let sql = r#"
            SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name = $1
            );
        "#;

        let exists: bool = sqlx::query_scalar(sql)
            .bind(table_name)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!(
                    "Failed to check if table '{table_name}' exists: {e}"
                ))
            })?;

        Ok(exists)
    }

    /// Get all tables in the database
    async fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let sql = r#"
            SELECT 
                table_name,
                (SELECT COUNT(*) FROM information_schema.columns 
                 WHERE table_schema = 'public' AND table_name = t.table_name) as column_count
            FROM information_schema.tables t
            WHERE table_schema = 'public'
            ORDER BY table_name;
        "#;

        let rows = sqlx::query(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| MigrationError::database(format!("Failed to get tables: {e}")))?;

        let mut tables = Vec::new();
        for row in rows {
            tables.push(TableInfo {
                name: row.get("table_name"),
                column_count: row.get::<i64, _>("column_count") as u32,
            });
        }

        Ok(tables)
    }

    /// Get all indexes in the database
    async fn get_indexes(&self) -> Result<Vec<IndexInfo>> {
        let sql = r#"
            SELECT 
                indexname as index_name,
                tablename as table_name,
                indexdef as definition
            FROM pg_indexes
            WHERE schemaname = 'public'
            ORDER BY tablename, indexname;
        "#;

        let rows = sqlx::query(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| MigrationError::database(format!("Failed to get indexes: {e}")))?;

        let mut indexes = Vec::new();
        for row in rows {
            indexes.push(IndexInfo {
                name: row.get("index_name"),
                table_name: row.get("table_name"),
                definition: row.get("definition"),
            });
        }

        Ok(indexes)
    }

    /// Get all constraints in the database
    async fn get_constraints(&self) -> Result<Vec<ConstraintInfo>> {
        let sql = r#"
            SELECT 
                constraint_name,
                table_name,
                constraint_type
            FROM information_schema.table_constraints
            WHERE table_schema = 'public'
            ORDER BY table_name, constraint_name;
        "#;

        let rows = sqlx::query(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| MigrationError::database(format!("Failed to get constraints: {e}")))?;

        let mut constraints = Vec::new();
        for row in rows {
            constraints.push(ConstraintInfo {
                name: row.get("constraint_name"),
                table_name: row.get("table_name"),
                constraint_type: row.get("constraint_type"),
            });
        }

        Ok(constraints)
    }

    /// Get the current schema version
    async fn get_current_version(&self) -> Result<Option<i64>> {
        if !self.table_exists("schema_migrations").await? {
            return Ok(None);
        }

        let sql = "SELECT MAX(version) FROM schema_migrations";

        let version: Option<i64> = sqlx::query_scalar(sql)
            .fetch_optional(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to get current schema version: {e}"))
            })?
            .flatten();

        Ok(version)
    }

    /// Get the last migration time
    async fn get_last_migration_time(&self) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
        if !self.table_exists("schema_migrations").await? {
            return Ok(None);
        }

        let sql = "SELECT MAX(applied_at) FROM schema_migrations";

        let timestamp: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(sql)
            .fetch_optional(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to get last migration time: {e}"))
            })?
            .flatten();

        Ok(timestamp)
    }

    /// Validate the schema_migrations table structure
    async fn validate_migrations_table(&self) -> Result<bool> {
        let sql = r#"
            SELECT column_name, data_type, is_nullable
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = 'schema_migrations'
            ORDER BY ordinal_position;
        "#;

        let rows = sqlx::query(sql)
            .fetch_all(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to validate migrations table: {e}"))
            })?;

        let expected_columns = vec![
            ("version", "bigint", "NO"),
            ("name", "character varying", "NO"),
            ("applied_at", "timestamp with time zone", "NO"),
            ("checksum", "character varying", "YES"),
            ("execution_time_ms", "bigint", "YES"),
        ];

        if rows.len() != expected_columns.len() {
            return Ok(false);
        }

        for (i, row) in rows.iter().enumerate() {
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let is_nullable: String = row.get("is_nullable");

            let (expected_name, expected_type, expected_nullable) = &expected_columns[i];

            if column_name != *expected_name
                || !data_type.contains(expected_type)
                || is_nullable != *expected_nullable
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get database statistics
    pub async fn get_database_stats(&self) -> Result<DatabaseStats> {
        let table_count = self.get_table_count().await?;
        let index_count = self.get_index_count().await?;
        let constraint_count = self.get_constraint_count().await?;

        Ok(DatabaseStats {
            table_count,
            index_count,
            constraint_count,
        })
    }

    /// Get the number of tables
    async fn get_table_count(&self) -> Result<u32> {
        let sql = r#"
            SELECT COUNT(*) 
            FROM information_schema.tables 
            WHERE table_schema = 'public';
        "#;

        let count: i64 = sqlx::query_scalar(sql)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| MigrationError::database(format!("Failed to get table count: {e}")))?;

        Ok(count as u32)
    }

    /// Get the number of indexes
    async fn get_index_count(&self) -> Result<u32> {
        let sql = r#"
            SELECT COUNT(*) 
            FROM pg_indexes 
            WHERE schemaname = 'public';
        "#;

        let count: i64 = sqlx::query_scalar(sql)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| MigrationError::database(format!("Failed to get index count: {e}")))?;

        Ok(count as u32)
    }

    /// Get the number of constraints
    async fn get_constraint_count(&self) -> Result<u32> {
        let sql = r#"
            SELECT COUNT(*) 
            FROM information_schema.table_constraints 
            WHERE table_schema = 'public';
        "#;

        let count: i64 = sqlx::query_scalar(sql)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| {
                MigrationError::database(format!("Failed to get constraint count: {e}"))
            })?;

        Ok(count as u32)
    }
}

/// Schema information
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct SchemaInfo {
    /// List of tables
    pub tables: Vec<TableInfo>,
    /// List of indexes
    pub indexes: Vec<IndexInfo>,
    /// List of constraints
    pub constraints: Vec<ConstraintInfo>,
    /// Current schema version
    pub version: Option<i64>,
    /// Last migration timestamp
    pub last_migration_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Table information
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct TableInfo {
    /// Table name
    pub name: String,
    /// Number of columns
    pub column_count: u32,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct IndexInfo {
    /// Index name
    pub name: String,
    /// Table name
    pub table_name: String,
    /// Index definition
    pub definition: String,
}

/// Constraint information
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ConstraintInfo {
    /// Constraint name
    pub name: String,
    /// Table name
    pub table_name: String,
    /// Constraint type
    pub constraint_type: String,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct DatabaseStats {
    /// Number of tables
    pub table_count: u32,
    /// Number of indexes
    pub index_count: u32,
    /// Number of constraints
    pub constraint_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_schema_info_creation() {
        let schema_info = SchemaInfo {
            tables: vec![],
            indexes: vec![],
            constraints: vec![],
            version: Some(1),
            last_migration_at: None,
        };

        assert_eq!(schema_info.version, Some(1));
        assert!(schema_info.tables.is_empty());
    }

    #[test]
    fn test_table_info() {
        let table_info = TableInfo {
            name: "users".to_string(),
            column_count: 5,
        };

        assert_eq!(table_info.name, "users");
        assert_eq!(table_info.column_count, 5);
    }

    #[test]
    fn test_database_stats() {
        let stats = DatabaseStats {
            table_count: 10,
            index_count: 15,
            constraint_count: 8,
        };

        assert_eq!(stats.table_count, 10);
        assert_eq!(stats.index_count, 15);
        assert_eq!(stats.constraint_count, 8);
    }

    #[test]
    fn test_schema_setters() {
        use pretty_assertions::assert_eq;

        // Test SchemaInfo setters
        let fixture = SchemaInfo {
            tables: vec![],
            indexes: vec![],
            constraints: vec![],
            version: None,
            last_migration_at: None,
        }
        .version(5)
        .tables(vec![TableInfo {
            name: "users".to_string(),
            column_count: 4,
        }])
        .indexes(vec![IndexInfo {
            name: "idx_users_email".to_string(),
            table_name: "users".to_string(),
            definition: "CREATE INDEX idx_users_email ON users(email)".to_string(),
        }]);

        let actual = (fixture.version, fixture.tables.len(), fixture.indexes.len());
        let expected = (Some(5), 1, 1);
        assert_eq!(actual, expected);

        // Test TableInfo setters
        let table_fixture = TableInfo {
            name: "test".to_string(),
            column_count: 0,
        }
        .name("products")
        .column_count(8u32);

        let actual_table = (&table_fixture.name, table_fixture.column_count);
        let expected_table = (&"products".to_string(), 8);
        assert_eq!(actual_table, expected_table);

        // Test DatabaseStats setters
        let stats_fixture = DatabaseStats {
            table_count: 0,
            index_count: 0,
            constraint_count: 0,
        }
        .table_count(25u32)
        .index_count(40u32)
        .constraint_count(15u32);

        let actual_stats = (
            stats_fixture.table_count,
            stats_fixture.index_count,
            stats_fixture.constraint_count,
        );
        let expected_stats = (25, 40, 15);
        assert_eq!(actual_stats, expected_stats);
    }
}
