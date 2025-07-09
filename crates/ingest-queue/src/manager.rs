use async_trait::async_trait;
use chrono::Utc;
use ingest_core::Error as CoreError;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::QueueConfig;
use crate::storage::HybridQueueStorage;
use crate::tenant::SimpleTenantManager;
use crate::traits::{QueueOperations, QueueStats, QueueStorage, TenantManager};
use crate::types::*;

/// Simplified queue manager - merges all functionality into a single component
pub struct CoreQueueManager {
    storage: Arc<dyn QueueStorage>,
    tenant_manager: Arc<dyn TenantManager>,
    config: Arc<RwLock<QueueConfig>>,
    stats: Arc<RwLock<QueueStats>>,
}

impl CoreQueueManager {
    /// Creates a new queue manager instance
    pub fn new(
        storage: Arc<dyn QueueStorage>,
        tenant_manager: Arc<dyn TenantManager>,
        config: QueueConfig,
    ) -> Self {
        Self {
            storage,
            tenant_manager,
            config: Arc::new(RwLock::new(config)),
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }

    /// Factory method: creates manager with default components
    pub async fn with_defaults(
        postgres: Arc<ingest_storage::PostgresStorage>,
        redis: Arc<ingest_storage::RedisStorage>,
    ) -> Result<Self, CoreError> {
        let config = QueueConfig::default();
        let storage_config = crate::storage::StorageConfig {
            enable_cache: config.enable_cache,
            cache_ttl_seconds: config.cache_ttl_seconds,
            write_through: config.write_through,
            read_through: config.read_through,
        };

        let storage = Arc::new(HybridQueueStorage::new(postgres, redis, storage_config));

        let tenant_manager = Arc::new(SimpleTenantManager::new(
            config.max_tenants,
            config.quota_enforcement,
        ));

        Ok(Self::new(storage, tenant_manager, config))
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, CoreError> {
        // Simplified health check logic
        let stats = self.storage.get_stats().await?;
        Ok(stats.total_jobs < u64::MAX) // Basic liveness check
    }

    /// Updates configuration
    pub async fn update_config(&self, new_config: QueueConfig) -> Result<(), CoreError> {
        let mut config = self.config.write().await;
        *config = new_config;
        Ok(())
    }

    /// Gets current configuration
    pub async fn get_config(&self) -> QueueConfig {
        self.config.read().await.clone()
    }

    /// Cleans up expired jobs
    pub async fn cleanup_expired_jobs(&self) -> Result<u64, CoreError> {
        let config = self.config.read().await;
        let _max_age_hours = config.dead_letter_max_age_hours;

        // Simplified cleanup logic
        let expired_jobs = self
            .storage
            .list_jobs(None, Some(JobStatus::Failed), Some(1000))
            .await?;

        let cleaned = expired_jobs.len() as u64;
        for job in expired_jobs {
            self.storage.delete_job(&job.id).await?;
        }

        Ok(cleaned)
    }

    /// Internal method: checks flow control
    async fn check_flow_control(&self, tenant_id: &TenantId) -> Result<bool, CoreError> {
        let config = self.config.read().await;

        if !config.enable_backpressure {
            return Ok(true);
        }

        // Simplified flow control check
        let tenant_stats = self.tenant_manager.get_tenant_stats(tenant_id).await?;
        Ok(tenant_stats.active_jobs < config.max_jobs_per_tenant)
    }

    /// Internal method: selects next job (priority scheduling)
    async fn select_next_job(
        &self,
        tenant_id: Option<TenantId>,
    ) -> Result<Option<QueueJob>, CoreError> {
        let _config = self.config.read().await;

        // Simplified scheduling logic: priority order processing
        for priority in [
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
        ] {
            let jobs = self
                .storage
                .list_jobs(tenant_id.as_ref(), Some(JobStatus::Pending), Some(1))
                .await?;

            if let Some(job) = jobs.into_iter().find(|j| j.priority == priority) {
                return Ok(Some(job));
            }
        }

        Ok(None)
    }

    /// Internal method: updates statistics
    async fn update_stats(&self) -> Result<(), CoreError> {
        let storage_stats = self.storage.get_stats().await?;
        let mut stats = self.stats.write().await;
        *stats = storage_stats;
        Ok(())
    }
}

#[async_trait]
impl QueueOperations for CoreQueueManager {
    async fn enqueue(&self, mut job: QueueJob) -> Result<(), CoreError> {
        // Check tenant permissions
        if !self.tenant_manager.can_enqueue(&job.tenant_id).await? {
            return Err(CoreError::queue(format!(
                "Tenant {} quota exceeded",
                job.tenant_id
            )));
        }

        // Check flow control
        if !self.check_flow_control(&job.tenant_id).await? {
            return Err(CoreError::queue("Backpressure active".to_string()));
        }

        // Set default values
        if job.priority == JobPriority::Normal {
            let config = self.config.read().await;
            job.priority = JobPriority::from_value(config.default_priority);
        }

        job.status = JobStatus::Pending;

        // Store job
        self.storage.store_job(&job).await?;

        // Record tenant activity
        self.tenant_manager.record_job(&job.tenant_id, &job).await?;

        // Update statistics
        self.update_stats().await?;

        Ok(())
    }

    async fn dequeue(&self, tenant_id: Option<TenantId>) -> Result<Option<QueueJob>, CoreError> {
        let target_tenant = if let Some(tid) = tenant_id {
            Some(tid)
        } else {
            // Fair scheduling: get next tenant
            self.tenant_manager.get_next_tenant().await?
        };

        if let Some(mut job) = self.select_next_job(target_tenant).await? {
            // Update job status
            job.status = JobStatus::Active;

            self.storage.update_job(&job).await?;
            self.update_stats().await?;

            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn get_job(&self, job_id: &JobId) -> Result<Option<QueueJob>, CoreError> {
        self.storage.retrieve_job(job_id).await
    }

    async fn update_job_status(&self, job_id: &JobId, status: JobStatus) -> Result<(), CoreError> {
        if let Some(mut job) = self.storage.retrieve_job(job_id).await? {
            job.status = status.clone();

            match status {
                JobStatus::Completed => {
                    // Job completed - simplified handling
                }
                JobStatus::Failed => {
                    job.retry_count += 1;

                    // Check if retry is needed
                    let config = self.config.read().await;
                    if job.retry_count < config.max_retries {
                        job.status = JobStatus::Pending;
                        job.scheduled_for = Utc::now() + chrono::Duration::seconds(60); // Retry after 1 minute
                    }
                }
                _ => {}
            }

            self.storage.update_job(&job).await?;
            self.update_stats().await?;
        }

        Ok(())
    }

    async fn get_queue_stats(&self) -> Result<QueueStats, CoreError> {
        self.update_stats().await?;
        Ok(self.stats.read().await.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::TenantStats;
    use pretty_assertions::assert_eq;

    // Mock storage implementation
    struct MockStorage;

    #[async_trait]
    impl QueueStorage for MockStorage {
        async fn store_job(&self, _job: &QueueJob) -> Result<(), CoreError> {
            Ok(())
        }

        async fn retrieve_job(&self, _job_id: &JobId) -> Result<Option<QueueJob>, CoreError> {
            Ok(None)
        }

        async fn update_job(&self, _job: &QueueJob) -> Result<(), CoreError> {
            Ok(())
        }

        async fn delete_job(&self, _job_id: &JobId) -> Result<(), CoreError> {
            Ok(())
        }

        async fn list_jobs(
            &self,
            _tenant_id: Option<&TenantId>,
            _status: Option<JobStatus>,
            _limit: Option<usize>,
        ) -> Result<Vec<QueueJob>, CoreError> {
            Ok(vec![])
        }

        async fn get_stats(&self) -> Result<QueueStats, CoreError> {
            Ok(QueueStats::default())
        }
    }

    // Mock tenant manager
    struct MockTenantManager;

    #[async_trait]
    impl TenantManager for MockTenantManager {
        async fn can_enqueue(&self, _tenant_id: &TenantId) -> Result<bool, CoreError> {
            Ok(true)
        }

        async fn record_job(
            &self,
            _tenant_id: &TenantId,
            _job: &QueueJob,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_next_tenant(&self) -> Result<Option<TenantId>, CoreError> {
            Ok(Some(ingest_core::generate_id_with_prefix("tenant")))
        }

        async fn get_tenant_stats(&self, _tenant_id: &TenantId) -> Result<TenantStats, CoreError> {
            Ok(TenantStats::default())
        }
    }

    #[tokio::test]
    async fn test_core_queue_manager_creation() {
        let storage = Arc::new(MockStorage);
        let tenant_manager = Arc::new(MockTenantManager);
        let config = QueueConfig::default();

        let manager = CoreQueueManager::new(storage, tenant_manager, config);

        let health = manager.health_check().await.unwrap();
        assert_eq!(health, true);
    }

    #[tokio::test]
    async fn test_enqueue_basic_job() {
        let storage = Arc::new(MockStorage);
        let tenant_manager = Arc::new(MockTenantManager);
        let config = QueueConfig::default();

        let manager = CoreQueueManager::new(storage, tenant_manager, config);

        let job = QueueJob {
            id: ingest_core::generate_id_with_prefix("job"),
            tenant_id: ingest_core::generate_id_with_prefix("tenant"),
            function_id: ingest_core::generate_id_with_prefix("fn"),
            payload: serde_json::json!({"test": "data"}),
            priority: JobPriority::Normal,
            status: JobStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            created_at: Utc::now(),
            scheduled_for: Utc::now(),
            timeout: std::time::Duration::from_secs(300),
            metadata: crate::types::JobMetadata::new(),
            last_error: None,
            updated_at: Utc::now(),
        };

        let result = manager.enqueue(job).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_queue_stats() {
        let storage = Arc::new(MockStorage);
        let tenant_manager = Arc::new(MockTenantManager);
        let config = QueueConfig::default();

        let manager = CoreQueueManager::new(storage, tenant_manager, config);

        let stats = manager.get_queue_stats().await.unwrap();
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.pending_jobs, 0);
    }
}
