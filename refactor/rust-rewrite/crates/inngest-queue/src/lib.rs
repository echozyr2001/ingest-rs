use chrono::{DateTime, Utc};
use inngest_core::{Edge, Priority, StateId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;
use uuid::Uuid;

pub mod memory_queue;
pub mod redis_queue;

//pub mod consumer;
//pub use consumer::*;

pub use memory_queue::*;
pub use redis_queue::*;

// TODO: Implement when we add Redis support
// pub mod redis_queue;

/// Queue item representing work to be processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier for this queue item
    pub id: Ulid,
    /// State identifier
    pub identifier: StateId,
    /// Edge being processed
    pub edge: Option<Edge>,
    /// Type of queue operation
    pub kind: QueueItemKind,
    /// Current attempt number (0-based)
    pub attempt: u32,
    /// Maximum number of attempts
    pub max_attempts: Option<u32>,
    /// Priority of this item
    pub priority: Priority,
    /// When this item should be processed (for delayed items)
    pub available_at: DateTime<Utc>,
    /// Group ID for related items
    pub group_id: Option<String>,
    /// Payload data
    pub payload: Option<serde_json::Value>,
    /// Run info populated by queue consumer
    pub run_info: Option<RunInfo>,
}

/// Types of queue operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueItemKind {
    /// Start a new function execution
    Start,
    /// Execute a step edge
    Edge,
    /// Handle a sleep/wait
    Sleep,
    /// Handle an error edge
    EdgeError,
    /// Handle a pause timeout
    Pause,
    /// Handle debounce logic
    Debounce,
    /// Schedule a batch
    ScheduleBatch,
    /// Cancel a function
    Cancel,
    /// Queue migration operation
    QueueMigrate,
}

/// Information about the queue run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    /// Queue latency
    pub latency: Duration,
    /// Sojourn delay in queue
    pub sojourn_delay: Duration,
    /// Item priority
    pub priority: Priority,
    /// Queue shard name
    pub queue_shard_name: String,
    /// Number of continues processed
    pub continue_count: u32,
    /// Whether refilled from backlog
    pub refilled_from_backlog: Option<String>,
}

/// Result of processing a queue item
#[derive(Debug, Clone)]
pub struct RunResult {
    /// Whether an immediate job was scheduled in same partition
    pub scheduled_immediate_job: bool,
}

/// Function signature for processing queue items
pub type RunFunc = Box<dyn Fn(RunInfo, QueueItem) -> Result<RunResult, QueueError> + Send + Sync>;

/// Queue error types
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Queue full")]
    QueueFull,

    #[error("Item not found: {id}")]
    ItemNotFound { id: Ulid },

    #[error("Invalid queue operation: {reason}")]
    InvalidOperation { reason: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Producer interface for adding items to queue
#[async_trait::async_trait]
pub trait Producer {
    /// Enqueue a single item
    async fn enqueue(&self, item: QueueItem) -> Result<(), QueueError>;

    /// Enqueue multiple items
    async fn enqueue_batch(&self, items: Vec<QueueItem>) -> Result<(), QueueError>;

    /// Schedule an item for future processing
    async fn schedule(&self, item: QueueItem, delay: Duration) -> Result<(), QueueError>;
}

/// Consumer interface for processing queue items
#[async_trait::async_trait]
pub trait Consumer {
    /// Run the consumer with a processing function
    async fn run(&self, processor: ProcessorFn) -> Result<(), QueueError>;

    /// Stop the consumer
    async fn stop(&self) -> Result<(), QueueError>;
}

/// Type alias for processor function to avoid generic complexity
pub type ProcessorFn =
    Arc<dyn Fn(RunInfo, QueueItem) -> Result<RunResult, QueueError> + Send + Sync>;

/// Queue reader interface for inspecting queue state
#[async_trait::async_trait]
pub trait QueueReader {
    /// Get queue length
    async fn len(&self) -> Result<usize, QueueError>;

    /// Check if queue is empty
    async fn is_empty(&self) -> Result<bool, QueueError>;

    /// Get items by function ID
    async fn get_items_by_function(&self, function_id: Uuid) -> Result<Vec<QueueItem>, QueueError>;
}

/// Combined queue interface
pub trait Queue: Producer + Consumer + QueueReader + Send + Sync {
    /// Set function paused state
    async fn set_function_paused(
        &self,
        account_id: Uuid,
        fn_id: Uuid,
        paused: bool,
    ) -> Result<(), QueueError>;
}
