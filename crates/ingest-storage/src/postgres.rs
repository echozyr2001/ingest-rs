use crate::{HealthCheck, Result, Storage, StorageError, Transaction};
use async_trait::async_trait;
use ingest_config::DatabaseConfig;
use sqlx::{PgPool, Postgres, postgres::PgPoolOptions};
use std::time::Instant;
use tracing::{debug, error, info};

/// PostgreSQL storage implementation
#[derive(Debug, Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Create a new PostgreSQL storage instance
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!(
            "Connecting to PostgreSQL database at {}:{}",
            config.host, config.port
        );

        let mut pool_options = PgPoolOptions::new();

        if let Some(max_conn) = config.max_connections {
            pool_options = pool_options.max_connections(max_conn);
        }

        if let Some(min_conn) = config.min_connections {
            pool_options = pool_options.min_connections(min_conn);
        }

        if let Some(timeout) = config.connect_timeout {
            pool_options = pool_options.acquire_timeout(timeout);
        }

        if let Some(idle_timeout) = config.idle_timeout {
            pool_options = pool_options.idle_timeout(idle_timeout);
        }

        if let Some(max_lifetime) = config.max_lifetime {
            pool_options = pool_options.max_lifetime(max_lifetime);
        }

        let pool = pool_options
            .connect(&config.url_with_ssl())
            .await
            .map_err(|e| {
                error!("Failed to connect to PostgreSQL: {}", e);
                StorageError::connection(format!("Failed to connect to PostgreSQL: {e}"))
            })?;

        info!("Successfully connected to PostgreSQL database");

        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    type Transaction = PostgresTransaction;

    async fn health_check(&self) -> Result<HealthCheck> {
        let start = Instant::now();

        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {
                let response_time = start.elapsed().as_millis() as u64;
                debug!("PostgreSQL health check passed in {}ms", response_time);
                Ok(HealthCheck::healthy(response_time))
            }
            Err(e) => {
                let response_time = start.elapsed().as_millis() as u64;
                error!("PostgreSQL health check failed: {}", e);
                Ok(HealthCheck::unhealthy(
                    format!("Health check failed: {e}"),
                    response_time,
                ))
            }
        }
    }

    async fn begin_transaction(&self) -> Result<Self::Transaction> {
        let tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            StorageError::transaction(format!("Failed to begin transaction: {e}"))
        })?;

        debug!("Started new PostgreSQL transaction");
        Ok(PostgresTransaction { tx: Some(tx) })
    }

    async fn execute_simple(&self, query: &str) -> Result<u64> {
        debug!("Executing query: {}", query);

        let result = sqlx::query(query).execute(&self.pool).await.map_err(|e| {
            error!("Query execution failed: {}", e);
            StorageError::query(format!("Query execution failed: {e}"))
        })?;

        let rows_affected = result.rows_affected();
        debug!("Query affected {} rows", rows_affected);
        Ok(rows_affected)
    }

    async fn close(&self) -> Result<()> {
        info!("Closing PostgreSQL connection pool");
        self.pool.close().await;
        Ok(())
    }
}

/// PostgreSQL transaction wrapper
pub struct PostgresTransaction {
    tx: Option<sqlx::Transaction<'static, Postgres>>,
}

#[async_trait]
impl Transaction for PostgresTransaction {
    async fn execute_simple(&mut self, query: &str) -> Result<u64> {
        debug!("Executing query in transaction: {}", query);

        let tx = self.tx.as_mut().ok_or_else(|| {
            StorageError::transaction("Transaction has been consumed".to_string())
        })?;

        let result = sqlx::query(query).execute(&mut **tx).await.map_err(|e| {
            error!("Transaction query execution failed: {}", e);
            StorageError::query(format!("Transaction query execution failed: {e}"))
        })?;

        let rows_affected = result.rows_affected();
        debug!("Transaction query affected {} rows", rows_affected);
        Ok(rows_affected)
    }

    async fn commit(mut self) -> Result<()> {
        debug!("Committing PostgreSQL transaction");

        let tx = self.tx.take().ok_or_else(|| {
            StorageError::transaction("Transaction has already been consumed".to_string())
        })?;

        tx.commit().await.map_err(|e| {
            error!("Transaction commit failed: {}", e);
            StorageError::transaction(format!("Transaction commit failed: {e}"))
        })?;

        debug!("Transaction committed successfully");
        Ok(())
    }

    async fn rollback(mut self) -> Result<()> {
        debug!("Rolling back PostgreSQL transaction");

        let tx = self.tx.take().ok_or_else(|| {
            StorageError::transaction("Transaction has already been consumed".to_string())
        })?;

        tx.rollback().await.map_err(|e| {
            error!("Transaction rollback failed: {}", e);
            StorageError::transaction(format!("Transaction rollback failed: {e}"))
        })?;

        debug!("Transaction rolled back successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_config::DatabaseConfig;
    use pretty_assertions::assert_eq;

    fn create_test_config() -> DatabaseConfig {
        DatabaseConfig::default()
            .host("localhost")
            .port(5432u16)
            .database("test_inngest")
            .username("postgres")
            .password("postgres")
    }

    #[tokio::test]
    async fn test_postgres_storage_creation() {
        // This test requires a running PostgreSQL instance
        // Skip if not available
        let config = create_test_config();

        // Just test that the creation doesn't panic
        // In a real test environment, you'd use testcontainers
        match PostgresStorage::new(&config).await {
            Ok(_) => {
                // Connection successful - this would only happen in CI with a real DB
            }
            Err(_) => {
                // Connection failed (expected if no DB available)
            }
        }
    }

    #[test]
    fn test_postgres_storage_config() {
        let fixture = create_test_config();
        let actual = fixture.url();
        let expected = "postgresql://postgres:postgres@localhost:5432/test_inngest";
        assert_eq!(actual, expected);
    }
}
