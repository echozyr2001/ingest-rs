use thiserror::Error;

/// Storage error types
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Connection error: {message}")]
    Connection { message: String },

    #[error("Transaction error: {message}")]
    Transaction { message: String },

    #[error("Query error: {message}")]
    Query { message: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Database error: {source}")]
    Database {
        #[from]
        source: sqlx::Error,
    },

    #[error("Core error: {source}")]
    Core {
        #[from]
        source: ingest_core::Error,
    },

    #[error("Config error: {source}")]
    Config {
        #[from]
        source: ingest_config::ConfigError,
    },
}

impl StorageError {
    /// Create a connection error
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Create a transaction error
    pub fn transaction(message: impl Into<String>) -> Self {
        Self::Transaction {
            message: message.into(),
        }
    }

    /// Create a query error
    pub fn query(message: impl Into<String>) -> Self {
        Self::Query {
            message: message.into(),
        }
    }

    /// Create a serialization error
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }
}

/// Result type alias for storage operations
pub type Result<T> = std::result::Result<T, StorageError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_storage_error_connection() {
        let fixture = "Failed to connect";
        let actual = StorageError::connection(fixture);
        assert!(matches!(actual, StorageError::Connection { .. }));
        assert_eq!(format!("{}", actual), "Connection error: Failed to connect");
    }

    #[test]
    fn test_storage_error_transaction() {
        let fixture = "Transaction failed";
        let actual = StorageError::transaction(fixture);
        assert!(matches!(actual, StorageError::Transaction { .. }));
        assert_eq!(
            format!("{}", actual),
            "Transaction error: Transaction failed"
        );
    }

    #[test]
    fn test_storage_error_query() {
        let fixture = "Query failed";
        let actual = StorageError::query(fixture);
        assert!(matches!(actual, StorageError::Query { .. }));
        assert_eq!(format!("{}", actual), "Query error: Query failed");
    }
}
