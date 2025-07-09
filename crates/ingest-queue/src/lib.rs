//! # ingest-queue
//!
//! Simplified high-performance queue system for the Inngest durable function platform.
//!
//! Refactored to reduce complexity, now provides:
//! - Unified configuration management
//! - Simplified core queue operations
//! - Basic multi-tenant support
//! - Hybrid storage layer
//!
//! ## Architecture
//!
//! The simplified queue system is built around several core components:
//!
//! - **CoreQueueManager**: Primary queue manager
//! - **QueueOperations**: Core queue operations trait
//! - **QueueStorage**: Storage layer abstraction
//! - **TenantManager**: Simplified tenant management
//!
//! ## Usage
//!
//! ```rust
//! use ingest_queue::{QueueJob, CoreQueueManager, QueueConfig};
//! use serde_json::json;
//!
//! # async fn example() -> anyhow::Result<(), ingest_core::Error> {
//! // Create configuration
//! let config = QueueConfig::default();
//!
//! // Create job
//! let job = QueueJob {
//! #   id: ingest_core::generate_id_with_prefix("job"),
//! #   tenant_id: ingest_core::generate_id_with_prefix("tenant"),
//! #   function_id: ingest_core::generate_id_with_prefix("fn"),
//! #   payload: json!({"message": "Hello, world!"}),
//! #   priority: ingest_queue::JobPriority::Normal,
//! #   status: ingest_queue::JobStatus::Pending,
//! #   retry_count: 0,
//! #   max_retries: 3,
//! #   created_at: chrono::Utc::now(),
//! #   scheduled_for: chrono::Utc::now(),
//! #   timeout: std::time::Duration::from_secs(300),
//! #   metadata: ingest_queue::JobMetadata::new(),
//! #   last_error: None,
//! #   updated_at: chrono::Utc::now(),
//!     // ... job fields
//! };
//!
//! // Use manager
//! // let manager = CoreQueueManager::with_defaults(postgres, redis).await?;
//! // manager.enqueue(job).await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod manager;
pub mod storage;
pub mod tenant;
pub mod traits;
pub mod types;

// Re-export common types and traits
pub use config::QueueConfig;
pub use manager::CoreQueueManager;
pub use traits::{QueueOperations, QueueStats, QueueStorage, TenantManager, TenantStats};
pub use types::{JobId, JobMetadata, JobPriority, JobStatus, QueueJob, TenantId};

// Re-export storage-related types
pub use storage::HybridQueueStorage;
pub use tenant::SimpleTenantManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::JobMetadata;
    use chrono::Utc;

    #[test]
    fn test_queue_job_creation() {
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let job_id = ingest_core::generate_id_with_prefix("job");
        let payload = serde_json::json!({"test": "data"});

        let job = QueueJob {
            id: job_id.clone(),
            tenant_id: tenant_id.clone(),
            function_id: ingest_core::generate_id_with_prefix("fn"),
            payload: payload.clone(),
            priority: JobPriority::Normal,
            status: JobStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            created_at: Utc::now(),
            scheduled_for: Utc::now(),
            timeout: std::time::Duration::from_secs(300),
            metadata: JobMetadata::new(),
            last_error: None,
            updated_at: Utc::now(),
        };

        assert_eq!(job.tenant_id, tenant_id);
        assert_eq!(job.id, job_id);
        assert_eq!(job.payload, payload);
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.priority, JobPriority::Normal);
    }

    #[test]
    fn test_priority_ordering() {
        let priorities = vec![
            JobPriority::Low,
            JobPriority::Critical,
            JobPriority::Normal,
            JobPriority::High,
        ];

        let mut sorted = priorities.clone();
        sorted.sort();

        assert_eq!(
            sorted,
            vec![
                JobPriority::Critical,
                JobPriority::High,
                JobPriority::Normal,
                JobPriority::Low,
            ]
        );
    }

    #[test]
    fn test_queue_config_default() {
        let config = QueueConfig::default();

        assert_eq!(config.max_concurrent_jobs, 100);
        assert_eq!(config.max_retries, 3);
        assert!(config.fair_scheduling);
        assert!(config.tenant_isolation);
        assert!(config.enable_cache);
    }
}
