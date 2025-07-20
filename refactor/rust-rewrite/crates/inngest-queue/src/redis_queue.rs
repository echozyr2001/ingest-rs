use async_trait::async_trait;
use chrono::Utc;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde_json;
use std::time::Duration;
use uuid::Uuid;

use crate::{Consumer, ProcessorFn, Producer, Queue, QueueError, QueueItem, QueueReader, RunInfo};

/// Redis-based queue implementation for production
#[derive(Clone)]
pub struct RedisQueue {
    connection_manager: ConnectionManager,
    queue_key: String,
    processing_key: String,
    dead_letter_key: String,
}

impl RedisQueue {
    /// Create a new Redis queue
    pub async fn new(redis_url: &str, queue_name: &str) -> Result<Self, QueueError> {
        let client = Client::open(redis_url)
            .map_err(|e| QueueError::Internal(format!("Failed to create Redis client: {}", e)))?;

        let connection_manager = ConnectionManager::new(client).await.map_err(|e| {
            QueueError::Internal(format!("Failed to create connection manager: {}", e))
        })?;

        Ok(Self {
            connection_manager,
            queue_key: format!("inngest:queue:{}", queue_name),
            processing_key: format!("inngest:processing:{}", queue_name),
            dead_letter_key: format!("inngest:dead_letter:{}", queue_name),
        })
    }

    /// Create connection from existing connection manager
    pub fn with_connection_manager(
        connection_manager: ConnectionManager,
        queue_name: &str,
    ) -> Self {
        Self {
            connection_manager,
            queue_key: format!("inngest:queue:{}", queue_name),
            processing_key: format!("inngest:processing:{}", queue_name),
            dead_letter_key: format!("inngest:dead_letter:{}", queue_name),
        }
    }

    /// Get queue health information
    pub async fn health_check(&self) -> Result<serde_json::Value, QueueError> {
        let mut conn = self.connection_manager.clone();

        let queue_len: i64 = conn
            .llen(&self.queue_key)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to get queue length: {}", e)))?;

        let processing_len: i64 = conn
            .llen(&self.processing_key)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to get processing length: {}", e)))?;

        let dead_letter_len: i64 = conn.llen(&self.dead_letter_key).await.map_err(|e| {
            QueueError::Internal(format!("Failed to get dead letter length: {}", e))
        })?;

        Ok(serde_json::json!({
            "queue_length": queue_len,
            "processing_length": processing_len,
            "dead_letter_length": dead_letter_len,
            "status": "healthy"
        }))
    }

    /// Move item to dead letter queue
    async fn move_to_dead_letter(&self, item: &QueueItem) -> Result<(), QueueError> {
        let mut conn = self.connection_manager.clone();

        let serialized = serde_json::to_string(item).map_err(|e| QueueError::Serialization(e))?;

        let _: () = conn
            .lpush(&self.dead_letter_key, &serialized)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to move to dead letter: {}", e)))?;

        Ok(())
    }

    /// Peek at next item without removing it
    pub async fn peek(&self) -> Result<Option<QueueItem>, QueueError> {
        let mut conn = self.connection_manager.clone();

        let item_data: Option<String> = conn
            .lindex(&self.queue_key, 0)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to peek: {}", e)))?;

        match item_data {
            Some(data) => {
                let item: QueueItem =
                    serde_json::from_str(&data).map_err(|e| QueueError::Serialization(e))?;
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// Simple push method for basic queuing (not using QueueItem structure)
    pub async fn push(&self, _id: &str, data: &str) -> Result<(), QueueError> {
        let mut conn = self.connection_manager.clone();

        let _: () = conn
            .lpush(&self.queue_key, data)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to push to queue: {}", e)))?;

        Ok(())
    }

    /// Simple ping method for health checks
    pub async fn ping(&self) -> Result<(), QueueError> {
        use redis::cmd;
        let mut conn = self.connection_manager.clone();

        let _: String = cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| QueueError::Internal(format!("Ping failed: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl Producer for RedisQueue {
    async fn enqueue(&self, item: QueueItem) -> Result<(), QueueError> {
        let mut conn = self.connection_manager.clone();

        let serialized = serde_json::to_string(&item).map_err(|e| QueueError::Serialization(e))?;

        // Use priority queue - higher priority items go to the front
        let _: () = match item.priority {
            inngest_core::Priority::High | inngest_core::Priority::Critical => {
                conn.lpush(&self.queue_key, &serialized).await
            }
            _ => conn.rpush(&self.queue_key, &serialized).await,
        }
        .map_err(|e| QueueError::Internal(format!("Failed to enqueue: {}", e)))?;

        Ok(())
    }

    async fn enqueue_batch(&self, items: Vec<QueueItem>) -> Result<(), QueueError> {
        if items.is_empty() {
            return Ok(());
        }

        let mut conn = self.connection_manager.clone();

        let mut high_priority = Vec::new();
        let mut normal_priority = Vec::new();

        for item in items {
            let serialized =
                serde_json::to_string(&item).map_err(|e| QueueError::Serialization(e))?;

            match item.priority {
                inngest_core::Priority::High => high_priority.push(serialized),
                _ => normal_priority.push(serialized),
            }
        }

        // Enqueue high priority items first (to front)
        if !high_priority.is_empty() {
            let args: Vec<&str> = std::iter::once(&*self.queue_key)
                .chain(high_priority.iter().map(|s| s.as_str()))
                .collect();

            let _: () = redis::cmd("LPUSH")
                .arg(&args)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    QueueError::Internal(format!("Failed to enqueue high priority batch: {}", e))
                })?;
        }

        // Enqueue normal priority items (to back)
        if !normal_priority.is_empty() {
            let args: Vec<&str> = std::iter::once(&*self.queue_key)
                .chain(normal_priority.iter().map(|s| s.as_str()))
                .collect();

            let _: () = redis::cmd("RPUSH")
                .arg(&args)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    QueueError::Internal(format!("Failed to enqueue normal priority batch: {}", e))
                })?;
        }

        Ok(())
    }

    async fn schedule(&self, item: QueueItem, delay: Duration) -> Result<(), QueueError> {
        let mut conn = self.connection_manager.clone();

        let execute_at = Utc::now()
            + chrono::Duration::from_std(delay)
                .map_err(|e| QueueError::Internal(format!("Invalid delay duration: {}", e)))?;

        let mut scheduled_item = item;
        scheduled_item.available_at = execute_at;

        let serialized =
            serde_json::to_string(&scheduled_item).map_err(|e| QueueError::Serialization(e))?;

        // Use sorted set for scheduled items
        let score = execute_at.timestamp_millis() as f64;
        let scheduled_key = format!("{}:scheduled", self.queue_key);

        let _: () = conn
            .zadd(&scheduled_key, &serialized, score)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to schedule: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl Consumer for RedisQueue {
    async fn run(&self, processor: ProcessorFn) -> Result<(), QueueError> {
        let mut conn = self.connection_manager.clone();

        loop {
            // First, check for scheduled items that are ready
            let now = Utc::now().timestamp_millis() as f64;
            let scheduled_key = format!("{}:scheduled", self.queue_key);

            let scheduled_items: Vec<String> = conn
                .zrangebyscore_limit(&scheduled_key, 0.0, now, 0, 10)
                .await
                .map_err(|e| {
                    QueueError::Internal(format!("Failed to get scheduled items: {}", e))
                })?;

            for item_data in scheduled_items {
                // Remove from scheduled set and add to main queue
                let _: i64 = conn.zrem(&scheduled_key, &item_data).await.map_err(|e| {
                    QueueError::Internal(format!("Failed to remove scheduled item: {}", e))
                })?;

                let _: i64 = conn.rpush(&self.queue_key, &item_data).await.map_err(|e| {
                    QueueError::Internal(format!("Failed to move scheduled to queue: {}", e))
                })?;
            }

            // Process items from main queue
            let item_data: Option<String> = conn
                .brpoplpush(&self.queue_key, &self.processing_key, 1.0)
                .await
                .map_err(|e| QueueError::Internal(format!("Failed to pop from queue: {}", e)))?;

            if let Some(data) = item_data {
                let item: QueueItem = match serde_json::from_str(&data) {
                    Ok(item) => item,
                    Err(e) => {
                        tracing::error!("Failed to deserialize queue item: {}", e);
                        // Remove malformed item from processing queue
                        let _: i64 = conn.lrem(&self.processing_key, 1, &data).await.unwrap_or(0);
                        continue;
                    }
                };

                let run_info = RunInfo {
                    latency: Duration::from_millis(0), // TODO: Calculate actual latency
                    sojourn_delay: Duration::from_millis(0), // TODO: Calculate sojourn delay
                    priority: item.priority.clone(),
                    queue_shard_name: "redis".to_string(),
                    continue_count: 0, // TODO: Track continues
                    refilled_from_backlog: None,
                };

                let result = processor(run_info, item.clone());

                match result {
                    Ok(_) => {
                        // Successfully processed, remove from processing queue
                        let _: i64 =
                            conn.lrem(&self.processing_key, 1, &data)
                                .await
                                .map_err(|e| {
                                    QueueError::Internal(format!(
                                        "Failed to remove processed item: {}",
                                        e
                                    ))
                                })?;
                    }
                    Err(e) => {
                        tracing::error!("Failed to process queue item: {}", e);

                        // Check if we should retry or move to dead letter
                        if item.attempt >= item.max_attempts.unwrap_or(3) {
                            // Move to dead letter queue
                            self.move_to_dead_letter(&item).await?;

                            // Remove from processing queue
                            let _: i64 =
                                conn.lrem(&self.processing_key, 1, &data)
                                    .await
                                    .map_err(|e| {
                                        QueueError::Internal(format!(
                                            "Failed to remove failed item: {}",
                                            e
                                        ))
                                    })?;
                        } else {
                            // Retry: increment attempt and put back in queue
                            let mut retry_item = item;
                            retry_item.attempt += 1;

                            // Add exponential backoff delay
                            let delay = Duration::from_secs(2_u64.pow(retry_item.attempt));
                            self.schedule(retry_item, delay).await?;

                            // Remove from processing queue
                            let _: i64 =
                                conn.lrem(&self.processing_key, 1, &data)
                                    .await
                                    .map_err(|e| {
                                        QueueError::Internal(format!(
                                            "Failed to remove retry item: {}",
                                            e
                                        ))
                                    })?;
                        }
                    }
                }
            }
        }
    }

    async fn stop(&self) -> Result<(), QueueError> {
        // In a real implementation, we would signal the consumer loop to stop
        // For now, this is a placeholder
        Ok(())
    }
}

#[async_trait]
impl QueueReader for RedisQueue {
    async fn len(&self) -> Result<usize, QueueError> {
        let mut conn = self.connection_manager.clone();

        let len: i64 = conn
            .llen(&self.queue_key)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to get queue length: {}", e)))?;

        Ok(len as usize)
    }

    async fn is_empty(&self) -> Result<bool, QueueError> {
        let len = self.len().await?;
        Ok(len == 0)
    }

    async fn get_items_by_function(
        &self,
        _function_id: Uuid,
    ) -> Result<Vec<QueueItem>, QueueError> {
        let mut conn = self.connection_manager.clone();

        // Get all items from queue
        let items_data: Vec<String> = conn
            .lrange(&self.queue_key, 0, -1)
            .await
            .map_err(|e| QueueError::Internal(format!("Failed to get all items: {}", e)))?;

        let mut filtered_items = Vec::new();

        for data in items_data {
            let item: QueueItem =
                serde_json::from_str(&data).map_err(|e| QueueError::Serialization(e))?;

            // TODO: Add function_id to QueueItem to enable proper filtering
            // For now, return all items
            filtered_items.push(item);
        }

        Ok(filtered_items)
    }
}

impl Queue for RedisQueue {
    async fn set_function_paused(
        &self,
        _account_id: Uuid,
        _fn_id: Uuid,
        _paused: bool,
    ) -> Result<(), QueueError> {
        // TODO: Implement function pausing in Redis
        // Could use a Redis set to track paused functions
        Ok(())
    }
}
