use crate::{HealthCheck, Result};
use async_trait::async_trait;

/// Storage trait for database operations
#[async_trait]
pub trait Storage: Send + Sync {
    /// Type of transaction this storage provides
    type Transaction: Transaction;

    /// Perform a health check
    async fn health_check(&self) -> Result<HealthCheck>;

    /// Begin a new transaction
    async fn begin_transaction(&self) -> Result<Self::Transaction>;

    /// Execute a simple query without parameters
    async fn execute_simple(&self, query: &str) -> Result<u64>;

    /// Close the storage connection
    async fn close(&self) -> Result<()>;
}

/// Transaction trait for database transactions
#[async_trait]
pub trait Transaction: Send {
    /// Execute a simple query within the transaction
    async fn execute_simple(&mut self, query: &str) -> Result<u64>;

    /// Commit the transaction
    async fn commit(self) -> Result<()>;

    /// Rollback the transaction
    async fn rollback(self) -> Result<()>;
}

#[cfg(test)]
mod tests {

    // Note: These are trait definitions, so we can only test that they compile
    // Actual implementation tests will be in the concrete storage types

    #[test]
    fn test_traits_compile() {
        // This test just ensures the traits compile correctly
        todo!("Ensure Storage and Transaction traits compile");
    }
}
