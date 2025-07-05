use thiserror::Error;

/// Migration error types
#[derive(Error, Debug)]
pub enum MigrationError {
    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Migration not found
    #[error("Migration not found: {0}")]
    NotFound(String),

    /// Invalid migration
    #[error("Invalid migration: {0}")]
    Invalid(String),

    /// Migration already applied
    #[error("Migration already applied: {0}")]
    AlreadyApplied(String),

    /// Migration dependency error
    #[error("Migration dependency error: {0}")]
    Dependency(String),

    /// Schema validation error
    #[error("Schema validation error: {0}")]
    Schema(String),

    /// SQL execution error
    #[error("SQL execution error: {0}")]
    Sql(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic error
    #[error("Migration error: {0}")]
    Other(String),
}

impl MigrationError {
    /// Create a database error
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create a not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create an invalid migration error
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    /// Create an already applied error
    pub fn already_applied(msg: impl Into<String>) -> Self {
        Self::AlreadyApplied(msg.into())
    }

    /// Create a dependency error
    pub fn dependency(msg: impl Into<String>) -> Self {
        Self::Dependency(msg.into())
    }

    /// Create a schema validation error
    pub fn schema(msg: impl Into<String>) -> Self {
        Self::Schema(msg.into())
    }

    /// Create an SQL execution error
    pub fn sql(msg: impl Into<String>) -> Self {
        Self::Sql(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Result type for migration operations
pub type Result<T> = std::result::Result<T, MigrationError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_migration_error_database() {
        let error = MigrationError::database("connection failed");
        assert_eq!(error.to_string(), "Database error: connection failed");
    }

    #[test]
    fn test_migration_error_not_found() {
        let error = MigrationError::not_found("migration 001");
        assert_eq!(error.to_string(), "Migration not found: migration 001");
    }

    #[test]
    fn test_migration_error_invalid() {
        let error = MigrationError::invalid("malformed SQL");
        assert_eq!(error.to_string(), "Invalid migration: malformed SQL");
    }

    #[test]
    fn test_result_type() {
        let failure: Result<i32> = Err(MigrationError::other("test"));
        assert!(failure.is_err());
    }
}
