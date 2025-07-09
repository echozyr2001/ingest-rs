//! Core types for the queue system
//!
//! This module defines the fundamental types used throughout the queue system,
//! including jobs, priorities, tenant information, and resource quotas.

use chrono::{DateTime, Utc};
use derive_setters::Setters;
use ingest_core::{Id, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Unique identifier for a queue job
pub type JobId = Id;

/// Unique identifier for a tenant
pub type TenantId = Id;

/// Unique identifier for a function
pub type FunctionId = Id;

/// Priority levels for queue jobs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobPriority {
    /// System-level jobs (highest priority)
    Critical = 0,
    /// User-defined high priority
    High = 1,
    /// Default priority
    Normal = 4,
    /// Background/cleanup jobs (lowest priority)
    Low = 7,
}

impl JobPriority {
    /// Get the numeric value of the priority
    pub fn value(&self) -> u8 {
        *self as u8
    }

    /// Create priority from numeric value
    pub fn from_value(value: u8) -> Self {
        match value {
            0 => Self::Critical,
            1..=3 => Self::High,
            4..=6 => Self::Normal,
            _ => Self::Low,
        }
    }
}

impl Default for JobPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Status of a queue job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is waiting to be processed
    Pending,
    /// Job is currently being processed
    Active,
    /// Job completed successfully
    Completed,
    /// Job failed and will be retried
    Failed,
    /// Job failed permanently and moved to dead letter queue
    DeadLetter,
    /// Job is scheduled for future execution
    Scheduled,
}

/// Metadata associated with a job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JobMetadata {
    /// Custom metadata fields
    pub fields: HashMap<String, String>,
    /// Source that created the job
    pub source: Option<String>,
    /// Job version for compatibility
    pub version: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl JobMetadata {
    /// Create new empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a metadata field
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the source
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// A job in the queue system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct QueueJob {
    /// Unique job identifier
    pub id: JobId,
    /// Tenant that owns this job
    pub tenant_id: TenantId,
    /// Function to execute
    pub function_id: FunctionId,
    /// Job priority
    pub priority: JobPriority,
    /// Job payload
    pub payload: Json,
    /// Current status
    pub status: JobStatus,
    /// Job creation timestamp
    pub created_at: DateTime<Utc>,
    /// When the job should be executed
    pub scheduled_for: DateTime<Utc>,
    /// Current retry count
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Job execution timeout
    pub timeout: Duration,
    /// Job metadata
    pub metadata: JobMetadata,
    /// Last error message (if any)
    pub last_error: Option<String>,
    /// When the job was last updated
    pub updated_at: DateTime<Utc>,
}

impl QueueJob {
    /// Create a new queue job
    pub fn new(tenant_id: TenantId, function_id: FunctionId, payload: Json) -> Self {
        let now = Utc::now();
        Self {
            id: ingest_core::generate_id_with_prefix("job"),
            tenant_id,
            function_id,
            priority: JobPriority::default(),
            payload,
            status: JobStatus::Pending,
            created_at: now,
            scheduled_for: now,
            retry_count: 0,
            max_retries: 3,
            timeout: Duration::from_secs(300), // 5 minutes default
            metadata: JobMetadata::new(),
            last_error: None,
            updated_at: now,
        }
    }

    /// Check if the job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment the retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.updated_at = Utc::now();
    }

    /// Mark the job as failed with an error message
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = if self.can_retry() {
            JobStatus::Failed
        } else {
            JobStatus::DeadLetter
        };
        self.last_error = Some(error.into());
        self.updated_at = Utc::now();
    }

    /// Mark the job as completed
    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.last_error = None;
        self.updated_at = Utc::now();
    }

    /// Mark the job as active
    pub fn mark_active(&mut self) {
        self.status = JobStatus::Active;
        self.updated_at = Utc::now();
    }

    /// Check if the job is ready to be executed
    pub fn is_ready(&self) -> bool {
        match self.status {
            JobStatus::Pending | JobStatus::Failed => Utc::now() >= self.scheduled_for,
            _ => false,
        }
    }

    /// Get the next retry delay using exponential backoff
    pub fn next_retry_delay(&self) -> Duration {
        let base_delay = Duration::from_secs(1);
        let backoff_factor = 2_u32.pow(self.retry_count);
        let max_delay = Duration::from_secs(300); // 5 minutes max

        std::cmp::min(base_delay * backoff_factor, max_delay)
    }

    /// Schedule the job for retry
    pub fn schedule_retry(&mut self) {
        if self.can_retry() {
            let delay = self.next_retry_delay();
            self.scheduled_for = Utc::now() + chrono::Duration::from_std(delay).unwrap();
            self.status = JobStatus::Scheduled;
            self.updated_at = Utc::now();
        }
    }
}

/// Resource quota for a tenant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ResourceQuota {
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Maximum queued jobs
    pub max_queue_size: u32,
    /// Jobs per second rate limit
    pub rate_limit_per_second: u32,
    /// Memory limit in bytes
    pub memory_limit_bytes: u64,
    /// CPU limit as a percentage (0-100)
    pub cpu_limit_percent: u8,
}

impl ResourceQuota {
    /// Create a new resource quota
    pub fn new() -> Self {
        Self {
            max_concurrent_jobs: 100,
            max_queue_size: 1000,
            rate_limit_per_second: 10,
            memory_limit_bytes: 1024 * 1024 * 100, // 100MB
            cpu_limit_percent: 50,
        }
    }

    /// Create unlimited quota (for testing)
    pub fn unlimited() -> Self {
        Self {
            max_concurrent_jobs: u32::MAX,
            max_queue_size: u32::MAX,
            rate_limit_per_second: u32::MAX,
            memory_limit_bytes: u64::MAX,
            cpu_limit_percent: 100,
        }
    }
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics for a tenant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TenantMetrics {
    /// Number of active jobs
    pub active_jobs: u32,
    /// Number of queued jobs
    pub queued_jobs: u32,
    /// Number of completed jobs
    pub completed_jobs: u64,
    /// Number of failed jobs
    pub failed_jobs: u64,
    /// Total jobs processed
    pub total_jobs: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Current memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Current CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl TenantMetrics {
    /// Create new tenant metrics
    pub fn new() -> Self {
        Self {
            last_updated: Utc::now(),
            ..Default::default()
        }
    }

    /// Update metrics with job completion
    pub fn record_job_completion(&mut self, processing_time_ms: u64) {
        self.completed_jobs += 1;
        self.total_jobs += 1;
        self.active_jobs = self.active_jobs.saturating_sub(1);

        // Update average processing time
        let total_time = self.avg_processing_time_ms * (self.completed_jobs - 1) as f64;
        self.avg_processing_time_ms =
            (total_time + processing_time_ms as f64) / self.completed_jobs as f64;

        self.last_updated = Utc::now();
    }

    /// Update metrics with job failure
    pub fn record_job_failure(&mut self) {
        self.failed_jobs += 1;
        self.total_jobs += 1;
        self.active_jobs = self.active_jobs.saturating_sub(1);
        self.last_updated = Utc::now();
    }

    /// Update metrics with job start
    pub fn record_job_start(&mut self) {
        self.active_jobs += 1;
        self.queued_jobs = self.queued_jobs.saturating_sub(1);
        self.last_updated = Utc::now();
    }

    /// Update metrics with job enqueue
    pub fn record_job_enqueue(&mut self) {
        self.queued_jobs += 1;
        self.last_updated = Utc::now();
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_jobs == 0 {
            0.0
        } else {
            (self.completed_jobs as f64 / self.total_jobs as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_job_priority_ordering() {
        let fixture = vec![
            JobPriority::Low,
            JobPriority::Critical,
            JobPriority::Normal,
            JobPriority::High,
        ];

        let mut actual = fixture.clone();
        actual.sort();

        let expected = vec![
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_job_priority_from_value() {
        let fixture = vec![
            (0, JobPriority::Critical),
            (2, JobPriority::High),
            (5, JobPriority::Normal),
            (9, JobPriority::Low),
        ];

        for (value, expected) in fixture {
            let actual = JobPriority::from_value(value);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_queue_job_creation() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let function_id = ingest_core::generate_id_with_prefix("fn");
        let payload = json!({"test": "data"});

        let actual = QueueJob::new(tenant_id.clone(), function_id.clone(), payload.clone());

        assert_eq!(actual.tenant_id, tenant_id);
        assert_eq!(actual.function_id, function_id);
        assert_eq!(actual.payload, payload);
        assert_eq!(actual.status, JobStatus::Pending);
        assert_eq!(actual.priority, JobPriority::Normal);
        assert_eq!(actual.retry_count, 0);
        assert_eq!(actual.max_retries, 3);
    }

    #[test]
    fn test_queue_job_can_retry() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let function_id = ingest_core::generate_id_with_prefix("fn");
        let mut fixture = QueueJob::new(tenant_id, function_id, json!({}));

        fixture.status = JobStatus::Failed;
        assert!(fixture.can_retry());

        fixture.retry_count = 3;
        assert!(!fixture.can_retry());

        fixture.status = JobStatus::Completed;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_queue_job_mark_failed() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let function_id = ingest_core::generate_id_with_prefix("fn");
        let mut fixture = QueueJob::new(tenant_id, function_id, json!({}));

        fixture.mark_failed("Test error");

        assert_eq!(fixture.status, JobStatus::Failed);
        assert_eq!(fixture.last_error, Some("Test error".to_string()));
    }

    #[test]
    fn test_queue_job_mark_failed_dead_letter() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let function_id = ingest_core::generate_id_with_prefix("fn");
        let mut fixture = QueueJob::new(tenant_id, function_id, json!({}));
        fixture.retry_count = 3; // At max retries

        fixture.mark_failed("Final error");

        assert_eq!(fixture.status, JobStatus::DeadLetter);
        assert_eq!(fixture.last_error, Some("Final error".to_string()));
    }

    #[test]
    fn test_queue_job_next_retry_delay() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let function_id = ingest_core::generate_id_with_prefix("fn");
        let mut fixture = QueueJob::new(tenant_id, function_id, json!({}));

        // First retry: 1 second
        let actual = fixture.next_retry_delay();
        let expected = Duration::from_secs(1);
        assert_eq!(actual, expected);

        // Second retry: 2 seconds
        fixture.retry_count = 1;
        let actual = fixture.next_retry_delay();
        let expected = Duration::from_secs(2);
        assert_eq!(actual, expected);

        // Third retry: 4 seconds
        fixture.retry_count = 2;
        let actual = fixture.next_retry_delay();
        let expected = Duration::from_secs(4);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_resource_quota_creation() {
        let actual = ResourceQuota::new();

        assert_eq!(actual.max_concurrent_jobs, 100);
        assert_eq!(actual.max_queue_size, 1000);
        assert_eq!(actual.rate_limit_per_second, 10);
    }

    #[test]
    fn test_tenant_metrics_job_completion() {
        let mut fixture = TenantMetrics::new();
        fixture.active_jobs = 1;

        fixture.record_job_completion(1000);

        assert_eq!(fixture.completed_jobs, 1);
        assert_eq!(fixture.total_jobs, 1);
        assert_eq!(fixture.active_jobs, 0);
        assert_eq!(fixture.avg_processing_time_ms, 1000.0);
    }

    #[test]
    fn test_tenant_metrics_success_rate() {
        let mut fixture = TenantMetrics::new();

        fixture.completed_jobs = 8;
        fixture.failed_jobs = 2;
        fixture.total_jobs = 10;

        let actual = fixture.success_rate();
        let expected = 80.0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_job_metadata_builder() {
        let actual = JobMetadata::new()
            .with_field("key1", "value1")
            .with_tag("tag1")
            .with_source("api");

        assert_eq!(actual.fields.get("key1"), Some(&"value1".to_string()));
        assert_eq!(actual.tags, vec!["tag1"]);
        assert_eq!(actual.source, Some("api".to_string()));
    }
}
