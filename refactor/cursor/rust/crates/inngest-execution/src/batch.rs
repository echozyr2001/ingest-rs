//! Event batching system
//!
//! Handles batching of events based on size and time limits, similar to the Go implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use inngest_core::{Error, Event, Result, Ulid, Uuid};
use serde::{Deserialize, Serialize};
use std::time::Duration;
// use tokio::time::Instant; // Currently unused

/// Configuration for event batching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Maximum number of events in a batch
    pub max_size: usize,
    /// Maximum time to wait before processing a partial batch
    pub timeout: Duration,
    /// Optional key for partitioning batches
    pub key: Option<String>,
}

impl BatchConfig {
    pub fn new(max_size: usize, timeout: Duration) -> Self {
        Self {
            max_size,
            timeout,
            key: None,
        }
    }

    pub fn with_key(mut self, key: String) -> Self {
        self.key = Some(key);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.max_size > 1 && !self.timeout.is_zero()
    }
}

/// Represents an item in a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItem {
    pub account_id: Uuid,
    pub workspace_id: Uuid,
    pub app_id: Uuid,
    pub function_id: Uuid,
    pub function_version: i32,
    pub event_id: Ulid,
    pub event: Event,
    pub version: i32,
}

/// Result of appending to a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAppendResult {
    pub status: BatchStatus,
    pub batch_id: Option<Ulid>,
    pub batch_pointer_key: String,
}

/// Status of a batch operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Event was appended to existing batch
    Append,
    /// New batch was created
    New,
    /// Batch is full and ready for execution
    Full,
    /// Batch reached maximum size limit
    MaxSize,
    /// Batch has already started execution
    Started,
    /// Batch is ready for execution
    Ready,
    /// Batch does not exist
    Absent,
}

/// Options for scheduling batch execution
#[derive(Debug, Clone)]
pub struct ScheduleBatchOpts {
    pub batch_id: Ulid,
    pub batch_pointer: String,
    pub account_id: Uuid,
    pub workspace_id: Uuid,
    pub app_id: Uuid,
    pub function_id: Uuid,
    pub function_version: i32,
    pub at: DateTime<Utc>,
}

impl ScheduleBatchOpts {
    pub fn job_id(&self) -> String {
        format!("{}:{}", self.workspace_id, self.batch_id)
    }
}

/// Trait for batch management operations
#[async_trait]
pub trait BatchManager: Send + Sync {
    /// Append an item to a batch
    async fn append(&self, item: BatchItem, config: &BatchConfig) -> Result<BatchAppendResult>;

    /// Retrieve all items in a batch
    async fn retrieve_items(&self, function_id: Uuid, batch_id: Ulid) -> Result<Vec<BatchItem>>;

    /// Start execution of a batch
    async fn start_execution(
        &self,
        function_id: Uuid,
        batch_id: Ulid,
        batch_pointer: &str,
    ) -> Result<BatchStatus>;

    /// Schedule batch execution for later
    async fn schedule_execution(&self, opts: ScheduleBatchOpts) -> Result<()>;

    /// Delete batch-related keys
    async fn delete_keys(&self, function_id: Uuid, batch_id: Ulid) -> Result<()>;
}

/// Redis-based batch manager implementation
pub struct RedisBatchManager {
    client: redis::Client,
    size_limit: usize,
}

impl RedisBatchManager {
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            size_limit: crate::defaults::DEFAULT_BATCH_SIZE_LIMIT,
        }
    }

    pub fn with_size_limit(mut self, limit: usize) -> Self {
        self.size_limit = limit;
        self
    }

    fn batch_key(&self, function_id: Uuid, batch_id: Ulid) -> String {
        format!("inngest:batches:{}:{}", function_id, batch_id)
    }

    fn batch_metadata_key(&self, function_id: Uuid, batch_id: Ulid) -> String {
        format!("{}:metadata", self.batch_key(function_id, batch_id))
    }

    fn batch_pointer_key(&self, function_id: Uuid, key_hash: Option<&str>) -> String {
        match key_hash {
            Some(hash) => format!("inngest:batch_ptr:{}:{}", function_id, hash),
            None => format!("inngest:batch_ptr:{}", function_id),
        }
    }

    async fn evaluate_batch_key(
        &self,
        _config: &BatchConfig,
        _event: &Event,
    ) -> Result<Option<String>> {
        if let Some(key_expr) = &_config.key {
            // In a full implementation, this would use the expression evaluator
            // For now, we'll use a simple placeholder
            tracing::warn!("Batch key expressions not yet implemented: {}", key_expr);
            Ok(Some("default".to_string()))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl BatchManager for RedisBatchManager {
    async fn append(&self, item: BatchItem, config: &BatchConfig) -> Result<BatchAppendResult> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| Error::Database(format!("Redis connection failed: {}", e)))?;

        let batch_key_hash = self.evaluate_batch_key(config, &item.event).await?;
        let batch_pointer = self.batch_pointer_key(item.function_id, batch_key_hash.as_deref());

        // This is a simplified version - the full implementation would use Lua scripts
        // for atomic operations as in the Go version
        let new_batch_id = Ulid::new();

        // For this example, we'll implement a basic version
        // In production, this would use the same Lua scripts as the Go implementation
        let batch_id: Option<String> = redis::cmd("GET")
            .arg(&batch_pointer)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to get batch pointer: {}", e)))?;

        let batch_id = if let Some(id) = batch_id {
            Ulid::from_string(&id)
                .map_err(|e| Error::InvalidData(format!("Invalid batch ID: {}", e)))?
        } else {
            // Create new batch
            let _: () = redis::cmd("SET")
                .arg(&batch_pointer)
                .arg(new_batch_id.to_string())
                .query_async(&mut conn)
                .await
                .map_err(|e| Error::Database(format!("Failed to create batch pointer: {}", e)))?;
            new_batch_id
        };

        let batch_key = self.batch_key(item.function_id, batch_id);

        // Add item to batch
        let item_json = serde_json::to_string(&item)?;
        let batch_size: u64 = redis::cmd("RPUSH")
            .arg(&batch_key)
            .arg(&item_json)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to append to batch: {}", e)))?;

        let status = if batch_size == 1 {
            BatchStatus::New
        } else if batch_size >= config.max_size as u64 {
            // Mark as full and update pointer
            let _: () = redis::cmd("SET")
                .arg(&batch_pointer)
                .arg(Ulid::new().to_string())
                .query_async(&mut conn)
                .await
                .map_err(|e| Error::Database(format!("Failed to update batch pointer: {}", e)))?;

            BatchStatus::Full
        } else {
            BatchStatus::Append
        };

        Ok(BatchAppendResult {
            status,
            batch_id: Some(batch_id),
            batch_pointer_key: batch_pointer,
        })
    }

    async fn retrieve_items(&self, function_id: Uuid, batch_id: Ulid) -> Result<Vec<BatchItem>> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| Error::Database(format!("Redis connection failed: {}", e)))?;

        let batch_key = self.batch_key(function_id, batch_id);

        let items: Vec<String> = redis::cmd("LRANGE")
            .arg(&batch_key)
            .arg(0)
            .arg(-1)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to retrieve batch items: {}", e)))?;

        let mut batch_items = Vec::with_capacity(items.len());
        for item_json in items {
            let item: BatchItem = serde_json::from_str(&item_json)?;
            batch_items.push(item);
        }

        Ok(batch_items)
    }

    async fn start_execution(
        &self,
        function_id: Uuid,
        batch_id: Ulid,
        _batch_pointer: &str,
    ) -> Result<BatchStatus> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| Error::Database(format!("Redis connection failed: {}", e)))?;

        let metadata_key = self.batch_metadata_key(function_id, batch_id);

        // Check if already started
        let status: Option<String> = redis::cmd("HGET")
            .arg(&metadata_key)
            .arg("status")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to get batch status: {}", e)))?;

        if let Some(status) = status {
            if status == "started" {
                return Ok(BatchStatus::Started);
            }
        }

        // Mark as started
        let _: () = redis::cmd("HSET")
            .arg(&metadata_key)
            .arg("status")
            .arg("started")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to set batch status: {}", e)))?;

        Ok(BatchStatus::Ready)
    }

    async fn schedule_execution(&self, opts: ScheduleBatchOpts) -> Result<()> {
        // In a full implementation, this would integrate with the queue system
        // to schedule the batch execution
        tracing::info!(
            "Scheduling batch execution for batch_id: {} at: {}",
            opts.batch_id,
            opts.at
        );
        Ok(())
    }

    async fn delete_keys(&self, function_id: Uuid, batch_id: Ulid) -> Result<()> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| Error::Database(format!("Redis connection failed: {}", e)))?;

        let batch_key = self.batch_key(function_id, batch_id);
        let metadata_key = self.batch_metadata_key(function_id, batch_id);

        let _: u64 = redis::cmd("DEL")
            .arg(&batch_key)
            .arg(&metadata_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete batch keys: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_batch_config_validation() {
        let config = BatchConfig::new(5, Duration::from_secs(30));
        assert!(config.is_enabled());

        let config = BatchConfig::new(1, Duration::from_secs(30));
        assert!(!config.is_enabled());

        let config = BatchConfig::new(5, Duration::ZERO);
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_schedule_batch_opts() {
        let opts = ScheduleBatchOpts {
            batch_id: Ulid::new(),
            batch_pointer: "test".to_string(),
            account_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            app_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
            function_version: 1,
            at: Utc::now(),
        };

        let job_id = opts.job_id();
        assert!(job_id.contains(&opts.workspace_id.to_string()));
        assert!(job_id.contains(&opts.batch_id.to_string()));
    }
}
