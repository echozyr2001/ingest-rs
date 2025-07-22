//! Queue management for Inngest
//!
//! This module provides queue management capabilities for scheduling and executing
//! function runs, including support for delays, retries, concurrency control, and priority scheduling.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use inngest_core::{Result, Ulid, Uuid};
use inngest_state::StateId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Queue item kinds matching Go's queue.Kind constants
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueItemKind {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "edge")]
    Edge,
    #[serde(rename = "sleep")]
    Sleep,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "edge-error")]
    EdgeError,
    #[serde(rename = "debounce")]
    Debounce,
    #[serde(rename = "schedule-batch")]
    ScheduleBatch,
    #[serde(rename = "queue-migrate")]
    QueueMigrate,
    #[serde(rename = "pbf")]
    PauseBlockFlush,
    #[serde(rename = "jps")]
    JobPromote,
    #[serde(rename = "cancel")]
    Cancel,
}

impl QueueItemKind {
    /// Check if this kind can have its score promoted for better scheduling
    pub fn is_promotable(&self) -> bool {
        matches!(
            self,
            QueueItemKind::Start
                | QueueItemKind::Sleep
                | QueueItemKind::Edge
                | QueueItemKind::Pause
                | QueueItemKind::EdgeError
        )
    }

    /// Check if this is a step kind that affects function execution
    pub fn is_step_kind(&self) -> bool {
        matches!(
            self,
            QueueItemKind::Start
                | QueueItemKind::Edge
                | QueueItemKind::Sleep
                | QueueItemKind::EdgeError
        )
    }
}

/// Throttle configuration for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Throttle {
    /// Unique throttling key
    pub key: String,
    /// Rate limit
    pub limit: i32,
    /// Burst capacity
    pub burst: i32,
    /// Period in seconds
    pub period: i32,
    /// Unhashed key value for debugging
    #[serde(skip)]
    pub unhashed_key: String,
    /// Expression hash for throttle key
    pub key_expression_hash: String,
}

/// Singleton configuration for preventing multiple runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Singleton {
    /// Unique singleton key
    pub key: String,
    /// Singleton mode (e.g., "cancel", "skip")
    pub mode: String,
}

/// Custom concurrency key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomConcurrency {
    /// Evaluated concurrency key
    pub key: String,
    /// Expression hash
    pub hash: String,
    /// Concurrency limit
    pub limit: i32,
    /// Unhashed evaluated key value
    #[serde(skip)]
    pub unhashed_evaluated_key_value: String,
}

/// Queue item payload for different operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum QueuePayload {
    /// Edge execution payload
    Edge {
        /// Outgoing edge ID
        outgoing: String,
        /// Incoming edge ID  
        incoming: String,
    },
    /// Pause timeout payload
    PauseTimeout {
        /// Pause ID
        pause_id: Uuid,
        /// Event that should resume the pause
        event: String,
        /// Timeout duration
        timeout: Duration,
    },
    /// Job promotion payload
    JobPromote {
        /// Job ID to promote
        job_id: String,
        /// Original scheduled time
        scheduled_at: i64,
    },
    /// Generic payload for other operations
    Generic(serde_json::Value),
}

/// Individual queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique item ID (hashed)
    pub id: String,
    /// Original job ID before hashing
    pub job_id: Option<String>,
    /// Group ID for tracking related operations
    pub group_id: String,
    /// Workspace ID
    pub workspace_id: Uuid,
    /// Function ID
    pub function_id: Uuid,
    /// Queue item kind
    pub kind: QueueItemKind,
    /// State identifier
    pub state_id: StateId,
    /// Execution attempt number (0-indexed)
    pub attempt: i32,
    /// Maximum attempts allowed
    pub max_attempts: Option<i32>,
    /// Time when item should be executed (Unix timestamp milliseconds)
    pub at_ms: i64,
    /// Wall time when job should actually run
    pub wall_time_ms: i64,
    /// Time when item was enqueued
    pub enqueued_at: i64,
    /// Lease ID for processing prevention
    pub lease_id: Option<Ulid>,
    /// Earliest time this was peeked from queue
    pub earliest_peek_time: Option<i64>,
    /// Whether refilled from backlog
    pub refilled_from: Option<String>,
    /// Time when refilled from backlog
    pub refilled_at: Option<i64>,
    /// Queue item payload
    pub payload: QueuePayload,
    /// Item metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Queue name override
    pub queue_name: Option<String>,
    /// Custom idempotency period
    pub idempotency_period: Option<Duration>,
    /// Throttle configuration
    pub throttle: Option<Throttle>,
    /// Singleton configuration
    pub singleton: Option<Singleton>,
    /// Custom concurrency keys
    pub custom_concurrency_keys: Vec<CustomConcurrency>,
    /// Priority factor
    pub priority_factor: Option<i64>,
}

impl QueueItem {
    /// Create a new queue item
    pub fn new(
        function_id: Uuid,
        workspace_id: Uuid,
        state_id: StateId,
        kind: QueueItemKind,
        payload: QueuePayload,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            id: String::new(), // Will be set when job_id is assigned
            job_id: None,
            group_id: Ulid::new().to_string(),
            workspace_id,
            function_id,
            kind,
            state_id,
            attempt: 0,
            max_attempts: None,
            at_ms: now,
            wall_time_ms: now,
            enqueued_at: now,
            lease_id: None,
            earliest_peek_time: None,
            refilled_from: None,
            refilled_at: None,
            payload,
            metadata: HashMap::new(),
            queue_name: None,
            idempotency_period: None,
            throttle: None,
            singleton: None,
            custom_concurrency_keys: Vec::new(),
            priority_factor: None,
        }
    }

    /// Set the job ID and generate hashed item ID
    pub fn set_job_id(&mut self, job_id: String) {
        self.job_id = Some(job_id.clone());
        self.id = hash_id(&job_id);
    }

    /// Get maximum attempts (default if not set)
    pub fn get_max_attempts(&self) -> i32 {
        self.max_attempts.unwrap_or(3)
    }

    /// Check if item is currently leased
    pub fn is_leased(&self, now: DateTime<Utc>) -> bool {
        if let Some(lease_id) = self.lease_id {
            let lease_time = DateTime::from_timestamp(lease_id.timestamp_ms() as i64 / 1000, 0)
                .unwrap_or_else(|| Utc::now());
            lease_time > now
        } else {
            false
        }
    }

    /// Get the scheduling score for this item
    pub fn score(&self, now: DateTime<Utc>) -> i64 {
        if !self.kind.is_promotable() || self.requires_promotion(now) {
            return self.at_ms;
        }

        // Use run ID timestamp for older functions to run first
        let start_at = self.state_id.run_id.timestamp_ms() as i64;

        if start_at == 0 {
            return self.at_ms;
        }

        // Apply priority factor
        start_at - self.get_priority_factor()
    }

    /// Check if job requires promotion (scheduled far in the future)
    pub fn requires_promotion(&self, now: DateTime<Utc>) -> bool {
        if !self.kind.is_promotable() {
            return false;
        }

        // If more than 2 seconds in the future, consider for promotion
        let future_limit = now.timestamp_millis() + 2000;
        self.at_ms > future_limit
    }

    /// Get priority factor in milliseconds
    pub fn get_priority_factor(&self) -> i64 {
        match self.kind {
            QueueItemKind::Start | QueueItemKind::Edge | QueueItemKind::EdgeError => {
                if let Some(factor) = self.priority_factor {
                    factor * 1000 // Convert to milliseconds
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Calculate sojourn latency (delay due to concurrency limits)
    pub fn sojourn_latency(&self, now: DateTime<Utc>) -> Duration {
        if let Some(refilled_at) = self.refilled_at {
            // Time between enqueue and refill plus expected delay
            Duration::from_millis((refilled_at - self.enqueued_at) as u64) + self.expected_delay()
        } else if let Some(earliest_peek) = self.earliest_peek_time {
            Duration::from_millis((now.timestamp_millis() - earliest_peek) as u64)
        } else {
            Duration::ZERO
        }
    }

    /// Calculate processing latency (excluding sojourn)
    pub fn latency(&self, now: DateTime<Utc>) -> Duration {
        if let Some(refilled_at) = self.refilled_at {
            // Time between refill and processing
            Duration::from_millis((now.timestamp_millis() - refilled_at) as u64)
        } else {
            let sojourn = self.sojourn_latency(now);
            let total = Duration::from_millis((now.timestamp_millis() - self.wall_time_ms) as u64);
            total.saturating_sub(sojourn)
        }
    }

    /// Calculate expected delay (for future scheduled items)
    pub fn expected_delay(&self) -> Duration {
        if self.enqueued_at == 0 {
            return Duration::ZERO;
        }

        let delay_ms = std::cmp::max(0, self.at_ms - self.enqueued_at);
        Duration::from_millis(delay_ms as u64)
    }
}

/// Queue run information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    /// Processing latency
    pub latency: Duration,
    /// Sojourn delay (concurrency-related)
    pub sojourn_delay: Duration,
    /// Item priority
    pub priority: u32,
    /// Queue shard name
    pub queue_shard_name: String,
    /// Continue count for processing chains
    pub continue_count: u32,
    /// Backlog refill source
    pub refilled_from_backlog: Option<String>,
}

/// Result of queue item processing
#[derive(Debug, Clone)]
pub struct RunResult {
    /// Whether another immediate job was scheduled
    pub scheduled_immediate_job: bool,
}

/// Queue processing function type
pub type RunFunction = Box<dyn Fn(RunInfo, QueueItem) -> Result<RunResult> + Send + Sync>;

/// Error types for queue operations
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue item not found")]
    ItemNotFound,
    #[error("Queue is full")]
    QueueFull,
    #[error("Item already leased")]
    ItemLeased,
    #[error("Retry limit exceeded")]
    RetryLimitExceeded,
    #[error("Queue operation failed: {0}")]
    OperationFailed(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Redis error: {0}")]
    RedisError(String),
}

/// Enqueue options
#[derive(Debug, Clone, Default)]
pub struct EnqueueOptions {
    /// Pass through the job ID without hashing
    pub passthrough_job_id: bool,
    /// Force a specific queue shard name
    pub force_queue_shard_name: Option<String>,
    /// Normalize from backlog ID
    pub normalize_from_backlog_id: Option<String>,
}

/// Queue manager trait for queue operations
#[async_trait]
pub trait QueueManager: Send + Sync {
    /// Enqueue an item to be processed at the specified time
    async fn enqueue(
        &self,
        item: QueueItem,
        at: DateTime<Utc>,
        options: EnqueueOptions,
    ) -> Result<()>;

    /// Dequeue and process items with the provided function
    async fn run(&self, run_fn: RunFunction) -> Result<()>;

    /// Get count of outstanding jobs for a run
    async fn outstanding_job_count(
        &self,
        env_id: Uuid,
        function_id: Uuid,
        run_id: Ulid,
    ) -> Result<i32>;

    /// Get count of running jobs for a function
    async fn running_count(&self, function_id: Uuid) -> Result<i64>;

    /// Get jobs for a specific run
    async fn run_jobs(
        &self,
        shard_name: &str,
        workspace_id: Uuid,
        function_id: Uuid,
        run_id: Ulid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<QueueItem>>;

    /// Load a specific queue item by ID
    async fn load_queue_item(&self, shard: &str, item_id: &str) -> Result<Option<QueueItem>>;

    /// Remove a specific queue item
    async fn remove_queue_item(
        &self,
        shard: &str,
        partition_key: &str,
        item_id: &str,
    ) -> Result<()>;

    /// Set function paused state
    async fn set_function_paused(
        &self,
        account_id: Uuid,
        function_id: Uuid,
        paused: bool,
    ) -> Result<()>;
}

/// In-memory queue manager for testing and development
pub struct InMemoryQueueManager {
    items: std::sync::Arc<tokio::sync::RwLock<HashMap<String, QueueItem>>>,
    paused_functions: std::sync::Arc<tokio::sync::RwLock<HashMap<Uuid, bool>>>,
}

impl InMemoryQueueManager {
    pub fn new() -> Self {
        Self {
            items: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            paused_functions: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueueManager for InMemoryQueueManager {
    async fn enqueue(
        &self,
        mut item: QueueItem,
        at: DateTime<Utc>,
        _options: EnqueueOptions,
    ) -> Result<()> {
        // Update scheduling time
        item.at_ms = at.timestamp_millis();
        item.wall_time_ms = at.timestamp_millis();

        let mut items = self.items.write().await;
        items.insert(item.id.clone(), item);
        Ok(())
    }

    async fn run(&self, _run_fn: RunFunction) -> Result<()> {
        // TODO: Implement queue processing loop
        // This would continuously poll for items and execute the run function
        Ok(())
    }

    async fn outstanding_job_count(
        &self,
        _env_id: Uuid,
        function_id: Uuid,
        run_id: Ulid,
    ) -> Result<i32> {
        let items = self.items.read().await;
        let count = items
            .values()
            .filter(|item| item.function_id == function_id && item.state_id.run_id == run_id)
            .count() as i32;
        Ok(count)
    }

    async fn running_count(&self, function_id: Uuid) -> Result<i64> {
        let items = self.items.read().await;
        let count = items
            .values()
            .filter(|item| item.function_id == function_id)
            .count() as i64;
        Ok(count)
    }

    async fn run_jobs(
        &self,
        _shard_name: &str,
        workspace_id: Uuid,
        function_id: Uuid,
        run_id: Ulid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<QueueItem>> {
        let items = self.items.read().await;
        let mut matching_items: Vec<_> = items
            .values()
            .filter(|item| {
                item.workspace_id == workspace_id
                    && item.function_id == function_id
                    && item.state_id.run_id == run_id
            })
            .cloned()
            .collect();

        // Sort by at_ms for consistent ordering
        matching_items.sort_by_key(|item| item.at_ms);

        let start = offset as usize;
        let end = std::cmp::min(start + limit as usize, matching_items.len());

        Ok(matching_items.get(start..end).unwrap_or(&[]).to_vec())
    }

    async fn load_queue_item(&self, _shard: &str, item_id: &str) -> Result<Option<QueueItem>> {
        let items = self.items.read().await;
        Ok(items.get(item_id).cloned())
    }

    async fn remove_queue_item(
        &self,
        _shard: &str,
        _partition_key: &str,
        item_id: &str,
    ) -> Result<()> {
        let mut items = self.items.write().await;
        items.remove(item_id);
        Ok(())
    }

    async fn set_function_paused(
        &self,
        _account_id: Uuid,
        function_id: Uuid,
        paused: bool,
    ) -> Result<()> {
        let mut paused_functions = self.paused_functions.write().await;
        if paused {
            paused_functions.insert(function_id, true);
        } else {
            paused_functions.remove(&function_id);
        }
        Ok(())
    }
}

/// Redis-based queue manager (placeholder for full implementation)
#[allow(dead_code)]
pub struct RedisQueueManager {
    client: redis::Client,
}

impl RedisQueueManager {
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).map_err(|e| {
            inngest_core::Error::Redis(format!("Failed to create Redis client: {}", e))
        })?;

        Ok(Self { client })
    }

    #[allow(dead_code)]
    fn queue_key(&self, shard: &str, partition: &str) -> String {
        format!("inngest:q:{}:{}", shard, partition)
    }

    #[allow(dead_code)]
    fn item_key(&self, item_id: &str) -> String {
        format!("inngest:i:{}", item_id)
    }

    #[allow(dead_code)]
    fn function_pause_key(&self, account_id: Uuid, function_id: Uuid) -> String {
        format!("inngest:fp:{}:{}", account_id, function_id)
    }
}

#[async_trait]
impl QueueManager for RedisQueueManager {
    async fn enqueue(
        &self,
        _item: QueueItem,
        _at: DateTime<Utc>,
        _options: EnqueueOptions,
    ) -> Result<()> {
        // TODO: Implement Redis-based enqueueing with proper sharding and scoring
        Ok(())
    }

    async fn run(&self, _run_fn: RunFunction) -> Result<()> {
        // TODO: Implement Redis-based queue processing with leasing and retry logic
        Ok(())
    }

    async fn outstanding_job_count(
        &self,
        _env_id: Uuid,
        _function_id: Uuid,
        _run_id: Ulid,
    ) -> Result<i32> {
        // TODO: Implement Redis-based job counting
        Ok(0)
    }

    async fn running_count(&self, _function_id: Uuid) -> Result<i64> {
        // TODO: Implement Redis-based running count
        Ok(0)
    }

    async fn run_jobs(
        &self,
        _shard_name: &str,
        _workspace_id: Uuid,
        _function_id: Uuid,
        _run_id: Ulid,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<QueueItem>> {
        // TODO: Implement Redis-based job listing
        Ok(Vec::new())
    }

    async fn load_queue_item(&self, _shard: &str, _item_id: &str) -> Result<Option<QueueItem>> {
        // TODO: Implement Redis-based item loading
        Ok(None)
    }

    async fn remove_queue_item(
        &self,
        _shard: &str,
        _partition_key: &str,
        _item_id: &str,
    ) -> Result<()> {
        // TODO: Implement Redis-based item removal
        Ok(())
    }

    async fn set_function_paused(
        &self,
        _account_id: Uuid,
        _function_id: Uuid,
        _paused: bool,
    ) -> Result<()> {
        // TODO: Implement Redis-based function pausing
        Ok(())
    }
}

/// Hash a string ID using xxHash (like Go version)
fn hash_id(id: &str) -> String {
    let hash = seahash::hash(id.as_bytes());
    format!("{:x}", hash) // Convert to hex string
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_queue_item_creation() {
        let function_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let state_id = StateId {
            run_id: Ulid::new(),
            function_id,
            tenant: inngest_state::Tenant {
                account_id: Uuid::new_v4(),
                env_id: workspace_id,
                app_id: Uuid::new_v4(),
            },
        };

        let mut item = QueueItem::new(
            function_id,
            workspace_id,
            state_id.clone(),
            QueueItemKind::Start,
            QueuePayload::Generic(serde_json::json!({})),
        );

        item.set_job_id("test-job-123".to_string());

        assert_eq!(item.function_id, function_id);
        assert_eq!(item.workspace_id, workspace_id);
        assert_eq!(item.state_id, state_id);
        assert_eq!(item.kind, QueueItemKind::Start);
        assert_eq!(item.job_id, Some("test-job-123".to_string()));
        assert!(!item.id.is_empty());
    }

    #[test]
    fn test_queue_item_kind_properties() {
        assert!(QueueItemKind::Start.is_promotable());
        assert!(QueueItemKind::Edge.is_promotable());
        assert!(!QueueItemKind::Debounce.is_promotable());

        assert!(QueueItemKind::Start.is_step_kind());
        assert!(QueueItemKind::Edge.is_step_kind());
        assert!(!QueueItemKind::Pause.is_step_kind());
    }

    #[test]
    fn test_hash_id() {
        let id = "test-job-123";
        let hashed = hash_id(id);
        assert!(!hashed.is_empty());

        // Same input should produce same hash
        let hashed2 = hash_id(id);
        assert_eq!(hashed, hashed2);
    }

    #[tokio::test]
    async fn test_in_memory_queue_manager() {
        let queue = InMemoryQueueManager::new();
        let function_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let state_id = StateId {
            run_id: Ulid::new(),
            function_id,
            tenant: inngest_state::Tenant {
                account_id: Uuid::new_v4(),
                env_id: workspace_id,
                app_id: Uuid::new_v4(),
            },
        };

        let mut item = QueueItem::new(
            function_id,
            workspace_id,
            state_id.clone(),
            QueueItemKind::Start,
            QueuePayload::Generic(serde_json::json!({})),
        );
        item.set_job_id("test-job-123".to_string());

        // Test enqueue
        let at = Utc::now();
        queue
            .enqueue(item.clone(), at, EnqueueOptions::default())
            .await
            .unwrap();

        // Test outstanding job count
        let count = queue
            .outstanding_job_count(workspace_id, function_id, state_id.run_id)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Test load item
        let loaded_item = queue.load_queue_item("", &item.id).await.unwrap();
        assert!(loaded_item.is_some());

        // Test remove item
        queue.remove_queue_item("", "", &item.id).await.unwrap();
        let removed_item = queue.load_queue_item("", &item.id).await.unwrap();
        assert!(removed_item.is_none());
    }

    #[test]
    fn test_queue_item_scoring() {
        let mut item = QueueItem::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            StateId {
                run_id: Ulid::new(),
                function_id: Uuid::new_v4(),
                tenant: inngest_state::Tenant {
                    account_id: Uuid::new_v4(),
                    env_id: Uuid::new_v4(),
                    app_id: Uuid::new_v4(),
                },
            },
            QueueItemKind::Edge,
            QueuePayload::Generic(serde_json::json!({})),
        );

        let now = Utc::now();
        item.at_ms = now.timestamp_millis();

        // Test basic scoring
        let score = item.score(now);
        assert!(score > 0);

        // Test priority factor
        item.priority_factor = Some(10);
        let priority_score = item.score(now);
        assert!(priority_score != score);
    }
}
