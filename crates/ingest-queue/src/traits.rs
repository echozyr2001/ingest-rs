use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ingest_core::Error as CoreError;

use crate::types::*;

/// Core queue operations trait - simplified version
#[async_trait]
pub trait QueueOperations: Send + Sync {
    async fn enqueue(&self, job: QueueJob) -> Result<(), CoreError>;
    async fn dequeue(&self, tenant_id: Option<TenantId>) -> Result<Option<QueueJob>, CoreError>;
    async fn get_job(&self, job_id: &JobId) -> Result<Option<QueueJob>, CoreError>;
    async fn update_job_status(&self, job_id: &JobId, status: JobStatus) -> Result<(), CoreError>;
    async fn get_queue_stats(&self) -> Result<QueueStats, CoreError>;
}

/// Storage layer trait - simplified version
#[async_trait]
pub trait QueueStorage: Send + Sync {
    async fn store_job(&self, job: &QueueJob) -> Result<(), CoreError>;
    async fn retrieve_job(&self, job_id: &JobId) -> Result<Option<QueueJob>, CoreError>;
    async fn update_job(&self, job: &QueueJob) -> Result<(), CoreError>;
    async fn delete_job(&self, job_id: &JobId) -> Result<(), CoreError>;
    async fn list_jobs(
        &self,
        tenant_id: Option<&TenantId>,
        status: Option<JobStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<QueueJob>, CoreError>;
    async fn get_stats(&self) -> Result<QueueStats, CoreError>;
}

/// Tenant management trait - simplified version
#[async_trait]
pub trait TenantManager: Send + Sync {
    async fn can_enqueue(&self, tenant_id: &TenantId) -> Result<bool, CoreError>;
    async fn record_job(&self, tenant_id: &TenantId, job: &QueueJob) -> Result<(), CoreError>;
    async fn get_next_tenant(&self) -> Result<Option<TenantId>, CoreError>;
    async fn get_tenant_stats(&self, tenant_id: &TenantId) -> Result<TenantStats, CoreError>;
}

// Removed traits (over-abstraction):
// - PriorityScheduler -> handled directly in QueueOperations
// - FlowController -> integrated into QueueOperations
// - DeadLetterHandler -> integrated into QueueStorage
// - HealthChecker -> simplified to method calls
// - ConfigProvider -> use configuration structs directly
// - MetricsCollector -> integrated into individual components
// - CircuitBreaker -> simplified to internal logic

/// Queue statistics information
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_jobs: u64,
    pub pending_jobs: u64,
    pub running_jobs: u64,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub dead_letter_jobs: u64,
    pub tenant_count: usize,
    pub average_processing_time: f64,
    pub last_updated: DateTime<Utc>,
}

/// Tenant statistics information
#[derive(Debug, Clone)]
pub struct TenantStats {
    pub tenant_id: TenantId,
    pub active_jobs: u32,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub quota_used: f64,
    pub quota_limit: f64,
    pub last_activity: DateTime<Utc>,
}

impl Default for QueueStats {
    fn default() -> Self {
        Self {
            total_jobs: 0,
            pending_jobs: 0,
            running_jobs: 0,
            completed_jobs: 0,
            failed_jobs: 0,
            dead_letter_jobs: 0,
            tenant_count: 0,
            average_processing_time: 0.0,
            last_updated: Utc::now(),
        }
    }
}

impl Default for TenantStats {
    fn default() -> Self {
        use uuid::Uuid;
        Self {
            tenant_id: TenantId::new(Uuid::new_v4()),
            active_jobs: 0,
            completed_jobs: 0,
            failed_jobs: 0,
            quota_used: 0.0,
            quota_limit: 100.0,
            last_activity: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats::default();

        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.pending_jobs, 0);
        assert_eq!(stats.tenant_count, 0);
        assert_eq!(stats.average_processing_time, 0.0);
    }

    #[test]
    fn test_tenant_stats_default() {
        let stats = TenantStats::default();

        assert_eq!(stats.active_jobs, 0);
        assert_eq!(stats.completed_jobs, 0);
        assert_eq!(stats.quota_used, 0.0);
        assert_eq!(stats.quota_limit, 100.0);
    }
}
