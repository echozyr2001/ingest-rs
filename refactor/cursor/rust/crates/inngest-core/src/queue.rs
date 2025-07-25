//! Queue system types and utilities for Inngest

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;
use uuid::Uuid;

use crate::Identifier;

/// Queue item kinds that can be processed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Start a new function execution
    Start,
    /// Execute a step within a function
    Edge,
    /// Sleep for a specified duration
    Sleep,
    /// Pause execution and wait for an event
    Pause,
    /// Handle step error
    EdgeError,
    /// Wait for event timeout
    WaitTimeout,
    /// Invoke another function
    InvokeFunction,
}

/// Priority levels for queue items
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Queue item that can be enqueued and processed
/// This corresponds to the Go `queue.Item` struct
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier for this queue item
    pub id: String,

    /// When this item should be executed (milliseconds since epoch)
    pub at_ms: i64,

    /// Wall clock time when this was enqueued
    pub wall_time_ms: i64,

    /// Function ID this item relates to
    pub function_id: Uuid,

    /// Workspace ID
    pub workspace_id: Uuid,

    /// Account ID
    pub account_id: Uuid,

    /// Lease ID if this item is currently leased
    pub lease_id: Option<Ulid>,

    /// The actual queue data
    pub data: Item,

    /// When this was enqueued
    pub enqueued_at: i64,

    /// Earliest time this can be peeked
    pub earliest_peek_time: i64,
}

impl QueueItem {
    /// Create a new queue item
    pub fn new(data: Item, at: DateTime<Utc>) -> Self {
        let now_ms = Utc::now().timestamp_millis();
        let at_ms = at.timestamp_millis();

        Self {
            id: Self::generate_id(&data),
            at_ms,
            wall_time_ms: now_ms,
            function_id: data.identifier.workflow_id,
            workspace_id: data.workspace_id,
            account_id: data.identifier.account_id,
            lease_id: None,
            data,
            enqueued_at: now_ms,
            earliest_peek_time: at_ms,
        }
    }

    /// Generate a deterministic ID for the queue item
    fn generate_id(item: &Item) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(item.identifier.run_id.to_string().as_bytes());
        hasher.update(item.kind.to_string().as_bytes());
        hasher.update(item.group_id.as_bytes());

        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Calculate the score for queue ordering
    pub fn score(&self, _now: DateTime<Utc>) -> i64 {
        if !self.is_promotable() {
            return self.at_ms;
        }

        // Base score on run ID timestamp
        let start_at = self.data.identifier.run_id.timestamp_ms() as i64;
        start_at - self.data.priority_factor()
    }

    /// Check if this item can be promoted in the queue
    pub fn is_promotable(&self) -> bool {
        // Items with priority factors can be promoted
        self.data.identifier.priority_factor.is_some()
    }

    /// Get the execution time as DateTime
    pub fn execution_time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.at_ms).unwrap_or_else(Utc::now)
    }

    /// Check if this item is ready to be executed
    pub fn is_ready(&self, now: DateTime<Utc>) -> bool {
        self.at_ms <= now.timestamp_millis()
    }

    /// Check if this item is currently leased
    pub fn is_leased(&self) -> bool {
        self.lease_id.is_some()
    }
}

/// Core item data for queue processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// Optional job ID for tracking
    pub job_id: Option<String>,

    /// Group ID for batching related items
    pub group_id: String,

    /// Workspace ID
    pub workspace_id: Uuid,

    /// Kind of item (start, edge, sleep, etc.)
    pub kind: ItemKind,

    /// Run identifier
    pub identifier: Identifier,

    /// Current attempt number (starts at 0)
    pub attempt: u32,

    /// Maximum number of attempts allowed
    pub max_attempts: Option<u32>,

    /// Type-specific payload data
    pub payload: serde_json::Value,
}

impl Item {
    /// Create a new item
    pub fn new(
        kind: ItemKind,
        identifier: Identifier,
        workspace_id: Uuid,
        group_id: impl Into<String>,
    ) -> Self {
        Self {
            job_id: None,
            group_id: group_id.into(),
            workspace_id,
            kind,
            identifier,
            attempt: 0,
            max_attempts: None,
            payload: serde_json::Value::Null,
        }
    }

    /// Set the payload
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Set the job ID
    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }

    /// Set the maximum attempts
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = Some(max_attempts);
        self
    }

    /// Get the effective priority factor
    pub fn priority_factor(&self) -> i64 {
        self.identifier.effective_priority_factor()
    }

    /// Check if this item can be retried
    pub fn can_retry(&self) -> bool {
        match self.max_attempts {
            Some(max) => self.attempt < max,
            None => self.attempt < 3, // Default max attempts
        }
    }

    /// Increment the attempt counter
    pub fn increment_attempt(&mut self) {
        self.attempt += 1;
    }

    /// Check if this is the first attempt
    pub fn is_first_attempt(&self) -> bool {
        self.attempt == 0
    }
}

impl std::fmt::Display for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemKind::Start => write!(f, "start"),
            ItemKind::Edge => write!(f, "edge"),
            ItemKind::Sleep => write!(f, "sleep"),
            ItemKind::Pause => write!(f, "pause"),
            ItemKind::EdgeError => write!(f, "edge_error"),
            ItemKind::WaitTimeout => write!(f, "wait_timeout"),
            ItemKind::InvokeFunction => write!(f, "invoke_function"),
        }
    }
}

/// Queue configuration options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum number of items that can be in the queue
    pub max_size: Option<usize>,

    /// Default item timeout in milliseconds
    pub default_timeout_ms: u64,

    /// Maximum lease duration in milliseconds
    pub max_lease_duration_ms: u64,

    /// Number of shards for the queue
    pub num_shards: u32,

    /// Concurrency limits
    pub concurrency_limits: HashMap<String, u32>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: None,
            default_timeout_ms: 30_000,     // 30 seconds
            max_lease_duration_ms: 300_000, // 5 minutes
            num_shards: 16,
            concurrency_limits: HashMap::new(),
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueStats {
    /// Total number of items in the queue
    pub total_items: u64,

    /// Number of items ready for execution
    pub ready_items: u64,

    /// Number of items currently being processed
    pub processing_items: u64,

    /// Number of items scheduled for future execution
    pub scheduled_items: u64,

    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,

    /// Queue throughput (items per second)
    pub throughput: f64,
}

impl Default for QueueStats {
    fn default() -> Self {
        Self {
            total_items: 0,
            ready_items: 0,
            processing_items: 0,
            scheduled_items: 0,
            avg_processing_time_ms: 0.0,
            throughput: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_identifier() -> Identifier {
        Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            1,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
    }

    #[test]
    fn test_item_creation() {
        let identifier = create_test_identifier();
        let workspace_id = Uuid::new_v4();

        let item = Item::new(
            ItemKind::Start,
            identifier.clone(),
            workspace_id,
            "test-group",
        );

        assert_eq!(item.kind, ItemKind::Start);
        assert_eq!(item.identifier, identifier);
        assert_eq!(item.workspace_id, workspace_id);
        assert_eq!(item.group_id, "test-group");
        assert_eq!(item.attempt, 0);
        assert!(item.is_first_attempt());
    }

    #[test]
    fn test_queue_item_creation() {
        let identifier = create_test_identifier();
        let workspace_id = Uuid::new_v4();
        let item = Item::new(ItemKind::Start, identifier, workspace_id, "test-group");

        let queue_item = QueueItem::new(item.clone(), Utc::now());

        assert_eq!(queue_item.data, item);
        assert_eq!(queue_item.function_id, item.identifier.workflow_id);
        assert_eq!(queue_item.workspace_id, workspace_id);
        assert!(!queue_item.is_leased());
    }

    #[test]
    fn test_item_retry_logic() {
        let mut item = Item::new(
            ItemKind::Edge,
            create_test_identifier(),
            Uuid::new_v4(),
            "test-group",
        )
        .with_max_attempts(3);

        assert!(item.can_retry());
        assert_eq!(item.attempt, 0);

        item.increment_attempt();
        assert_eq!(item.attempt, 1);
        assert!(item.can_retry());

        item.increment_attempt();
        item.increment_attempt();
        assert_eq!(item.attempt, 3);
        assert!(!item.can_retry());
    }

    #[test]
    fn test_queue_item_readiness() {
        let item = Item::new(
            ItemKind::Start,
            create_test_identifier(),
            Uuid::new_v4(),
            "test-group",
        );

        let past = Utc::now() - chrono::Duration::minutes(1);
        let future = Utc::now() + chrono::Duration::minutes(1);
        let now = Utc::now();

        let past_item = QueueItem::new(item.clone(), past);
        let future_item = QueueItem::new(item.clone(), future);

        assert!(past_item.is_ready(now));
        assert!(!future_item.is_ready(now));
    }

    #[test]
    fn test_item_serialization() {
        let item = Item::new(
            ItemKind::Start,
            create_test_identifier(),
            Uuid::new_v4(),
            "test-group",
        )
        .with_payload(serde_json::json!({"key": "value"}));

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: Item = serde_json::from_str(&json).unwrap();

        assert_eq!(item, deserialized);
    }
}
