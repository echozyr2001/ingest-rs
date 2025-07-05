use crate::{Event, Result};
use async_trait::async_trait;

/// Event handler trait for processing events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: Event) -> Result<()>;

    /// Get the handler name for identification
    fn name(&self) -> &str;

    /// Check if this handler can process the given event
    fn can_handle(&self, event: &Event) -> bool;
}

/// State manager trait for managing execution state
#[async_trait]
pub trait StateManager: Send + Sync {
    /// Save state data
    async fn save_state(&self, key: &str, data: &crate::Json) -> Result<()>;

    /// Load state data
    async fn load_state(&self, key: &str) -> Result<Option<crate::Json>>;

    /// Delete state data
    async fn delete_state(&self, key: &str) -> Result<()>;

    /// Check if state exists
    async fn state_exists(&self, key: &str) -> Result<bool>;

    /// List all state keys with optional prefix
    async fn list_state_keys(&self, prefix: Option<&str>) -> Result<Vec<String>>;

    /// Clear all state (use with caution)
    async fn clear_all_state(&self) -> Result<()>;
}

/// Queue provider trait for job scheduling and execution
#[async_trait]
pub trait QueueProvider: Send + Sync {
    /// Enqueue a job for processing
    async fn enqueue(&self, queue: &str, job: QueueJob) -> Result<String>;

    /// Dequeue a job for processing
    async fn dequeue(&self, queue: &str) -> Result<Option<QueueJob>>;

    /// Acknowledge job completion
    async fn ack(&self, queue: &str, job_id: &str) -> Result<()>;

    /// Reject a job and optionally requeue it
    async fn reject(&self, queue: &str, job_id: &str, requeue: bool) -> Result<()>;

    /// Get queue statistics
    async fn queue_stats(&self, queue: &str) -> Result<QueueStats>;

    /// List all available queues
    async fn list_queues(&self) -> Result<Vec<String>>;

    /// Purge all jobs from a queue
    async fn purge_queue(&self, queue: &str) -> Result<u64>;
}

/// Job data for queue operations
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueueJob {
    /// Unique job identifier
    pub id: String,
    /// Job payload
    pub data: crate::Json,
    /// Job priority (higher = more priority)
    pub priority: i32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Current retry count
    pub retry_count: u32,
    /// Job creation timestamp
    pub created_at: crate::DateTime,
    /// Job scheduled execution time
    pub scheduled_at: Option<crate::DateTime>,
    /// Job metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl QueueJob {
    /// Create a new queue job
    pub fn new(data: crate::Json) -> Self {
        Self {
            id: crate::generate_id().into_string(),
            data,
            priority: 0,
            max_retries: 3,
            retry_count: 0,
            created_at: chrono::Utc::now(),
            scheduled_at: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Schedule job for future execution
    pub fn scheduled_at(mut self, at: crate::DateTime) -> Self {
        self.scheduled_at = Some(at);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Queue statistics
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueueStats {
    /// Queue name
    pub name: String,
    /// Number of pending jobs
    pub pending: u64,
    /// Number of active jobs
    pub active: u64,
    /// Number of completed jobs
    pub completed: u64,
    /// Number of failed jobs
    pub failed: u64,
    /// Number of delayed jobs
    pub delayed: u64,
}

impl QueueStats {
    /// Create new queue stats
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pending: 0,
            active: 0,
            completed: 0,
            failed: 0,
            delayed: 0,
        }
    }

    /// Get total number of jobs
    pub fn total(&self) -> u64 {
        self.pending + self.active + self.completed + self.failed + self.delayed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_queue_job_creation() {
        let fixture = json!({"task": "send_email", "user_id": 123});
        let actual = QueueJob::new(fixture.clone());

        assert_eq!(actual.data, fixture);
        assert_eq!(actual.priority, 0);
        assert_eq!(actual.max_retries, 3);
        assert_eq!(actual.retry_count, 0);
        assert!(actual.scheduled_at.is_none());
    }

    #[test]
    fn test_queue_job_with_priority() {
        let fixture = QueueJob::new(json!({})).with_priority(10);
        let actual = fixture.priority;
        let expected = 10;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_job_with_max_retries() {
        let fixture = QueueJob::new(json!({})).with_max_retries(5);
        let actual = fixture.max_retries;
        let expected = 5;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_job_scheduled_at() {
        let scheduled_time = chrono::Utc::now() + chrono::Duration::hours(1);
        let fixture = QueueJob::new(json!({})).scheduled_at(scheduled_time);
        let actual = fixture.scheduled_at;
        let expected = Some(scheduled_time);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_job_with_metadata() {
        let fixture = QueueJob::new(json!({}))
            .with_metadata("source", "api")
            .with_metadata("version", "1.0");

        assert_eq!(fixture.metadata.get("source"), Some(&"api".to_string()));
        assert_eq!(fixture.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_queue_job_can_retry() {
        let mut fixture = QueueJob::new(json!({}));

        assert!(fixture.can_retry());

        fixture.retry_count = 3;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_queue_job_increment_retry() {
        let mut fixture = QueueJob::new(json!({}));
        let original_count = fixture.retry_count;

        fixture.increment_retry();

        let actual = fixture.retry_count;
        let expected = original_count + 1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_stats_creation() {
        let fixture = "test-queue";
        let actual = QueueStats::new(fixture);
        let expected = QueueStats {
            name: "test-queue".to_string(),
            pending: 0,
            active: 0,
            completed: 0,
            failed: 0,
            delayed: 0,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_stats_total() {
        let mut fixture = QueueStats::new("test-queue");
        fixture.pending = 5;
        fixture.active = 2;
        fixture.completed = 10;
        fixture.failed = 1;
        fixture.delayed = 3;

        let actual = fixture.total();
        let expected = 21;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_queue_job_serialization() {
        let fixture = QueueJob::new(json!({"test": "data"}));
        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_queue_job_deserialization() {
        let fixture = QueueJob::new(json!({"test": "data"}));
        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: QueueJob = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.data, fixture.data);
        assert_eq!(actual.priority, fixture.priority);
    }

    #[test]
    fn test_queue_stats_serialization() {
        let fixture = QueueStats::new("test-queue");
        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_queue_stats_deserialization() {
        let fixture = QueueStats::new("test-queue");
        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: QueueStats = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.name, fixture.name);
        assert_eq!(actual.total(), fixture.total());
    }
}
