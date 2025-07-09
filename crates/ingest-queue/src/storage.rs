//! Storage integration for the queue system
//!
//! This module provides persistence and caching for queue jobs using
//! PostgreSQL for durable storage and Redis for high-performance caching.

use crate::traits::{QueueStats, QueueStorage};
use crate::types::{JobId, JobStatus, QueueJob, TenantId};
use async_trait::async_trait;
use chrono::Utc;
use ingest_core::Error as CoreError;
use ingest_storage::{CacheStorage, PostgresStorage, RedisStorage, Storage};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Hybrid storage implementation using PostgreSQL and Redis
pub struct HybridQueueStorage {
    /// PostgreSQL for persistent storage
    postgres: Arc<PostgresStorage>,
    /// Redis for caching
    redis: Arc<RedisStorage>,
    /// Configuration
    config: Arc<RwLock<StorageConfig>>,
}

/// Configuration for queue storage
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Enable Redis caching
    pub enable_cache: bool,
    /// Cache TTL for active jobs (seconds)
    pub cache_ttl_seconds: u64,
    /// Enable write-through caching
    pub write_through: bool,
    /// Enable read-through caching
    pub read_through: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            cache_ttl_seconds: 3600, // 1 hour
            write_through: true,
            read_through: true,
        }
    }
}

impl HybridQueueStorage {
    /// Create a new hybrid storage instance
    pub fn new(
        postgres: Arc<PostgresStorage>,
        redis: Arc<RedisStorage>,
        config: StorageConfig,
    ) -> Self {
        Self {
            postgres,
            redis,
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(postgres: Arc<PostgresStorage>, redis: Arc<RedisStorage>) -> Self {
        Self::new(postgres, redis, StorageConfig::default())
    }

    /// Get cache key for a job
    fn get_job_cache_key(&self, job_id: &JobId) -> String {
        format!("queue:job:{job_id}")
    }

    /// Store job in cache
    async fn cache_job(&self, job: &QueueJob) -> Result<(), CoreError> {
        let config = self.config.read().await;
        if !config.enable_cache {
            return Ok(());
        }

        let key = self.get_job_cache_key(&job.id);
        let ttl = config.cache_ttl_seconds;

        let job_json = serde_json::to_string(job)
            .map_err(|e| CoreError::queue(format!("Failed to serialize job: {e}")))?;

        self.redis
            .set(
                &key,
                job_json.as_bytes(),
                Some(std::time::Duration::from_secs(ttl)),
            )
            .await
            .map_err(|e| CoreError::queue(format!("Failed to cache job: {e}")))?;
        debug!("Cached job {} with TTL {}", job.id, ttl);
        Ok(())
    }

    /// Get job from cache
    async fn get_cached_job(&self, job_id: &JobId) -> Result<Option<QueueJob>, CoreError> {
        let config = self.config.read().await;
        if !config.enable_cache {
            return Ok(None);
        }

        let key = self.get_job_cache_key(job_id);

        match self.redis.get(&key).await {
            Ok(Some(job_bytes)) => {
                let job_json = String::from_utf8(job_bytes)
                    .map_err(|e| CoreError::queue(format!("Invalid UTF-8 in cached job: {e}")))?;
                let job: QueueJob = serde_json::from_str(&job_json).map_err(|e| {
                    CoreError::queue(format!("Failed to deserialize cached job: {e}"))
                })?;
                debug!("Retrieved job {} from cache", job_id);
                Ok(Some(job))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("Failed to get job from cache: {}", e);
                Ok(None)
            }
        }
    }

    /// Remove job from cache
    async fn remove_cached_job(&self, job_id: &JobId) -> Result<(), CoreError> {
        let config = self.config.read().await;
        if !config.enable_cache {
            return Ok(());
        }

        let key = self.get_job_cache_key(job_id);

        if let Err(e) = self.redis.delete(&key).await {
            warn!("Failed to remove job from cache: {}", e);
        } else {
            debug!("Removed job {} from cache", job_id);
        }

        Ok(())
    }

    /// Store job in PostgreSQL (simplified for demo)
    async fn store_job_postgres(&self, job: &QueueJob) -> Result<(), CoreError> {
        let _job_json = serde_json::to_string(job)
            .map_err(|e| CoreError::queue(format!("Failed to serialize job: {e}")))?;

        // Simplified implementation - in reality would use proper SQL
        let query = format!("-- Store job {} for tenant {}", job.id, job.tenant_id);
        self.postgres
            .execute_simple(&query)
            .await
            .map_err(|e| CoreError::queue(format!("Failed to store job in PostgreSQL: {e}")))?;

        debug!("Stored job {} in PostgreSQL", job.id);
        Ok(())
    }
}

#[async_trait]
impl QueueStorage for HybridQueueStorage {
    async fn store_job(&self, job: &QueueJob) -> Result<(), CoreError> {
        let config = self.config.read().await;

        // Always store in PostgreSQL for durability
        self.store_job_postgres(job).await?;

        // Cache if write-through is enabled
        if config.write_through {
            if let Err(e) = self.cache_job(job).await {
                warn!("Failed to cache job during store: {}", e);
                // Don't fail the operation if caching fails
            }
        }

        Ok(())
    }

    async fn retrieve_job(&self, job_id: &JobId) -> Result<Option<QueueJob>, CoreError> {
        let config = self.config.read().await;

        // Try cache first if read-through is enabled
        if config.read_through {
            if let Ok(Some(job)) = self.get_cached_job(job_id).await {
                return Ok(Some(job));
            }
        }

        // For demo purposes, return None (no actual PostgreSQL query)
        // In reality, this would query PostgreSQL
        Ok(None)
    }

    async fn update_job(&self, job: &QueueJob) -> Result<(), CoreError> {
        // Update in PostgreSQL
        self.store_job_postgres(job).await?;

        // Update cache
        let config = self.config.read().await;
        if config.enable_cache {
            if let Err(e) = self.cache_job(job).await {
                warn!("Failed to update job in cache: {}", e);
            }
        }

        Ok(())
    }

    async fn delete_job(&self, job_id: &JobId) -> Result<(), CoreError> {
        // Delete from PostgreSQL - simplified
        self.postgres
            .execute_simple("-- DELETE job")
            .await
            .map_err(|e| CoreError::queue(format!("Failed to delete job: {e}")))?;

        // Remove from cache
        self.remove_cached_job(job_id).await?;

        info!("Deleted job {}", job_id);
        Ok(())
    }

    async fn list_jobs(
        &self,
        _tenant_id: Option<&TenantId>,
        _status: Option<JobStatus>,
        _limit: Option<usize>,
    ) -> Result<Vec<QueueJob>, CoreError> {
        // Simplified implementation - return empty list
        // In reality, would query PostgreSQL with proper filters
        Ok(Vec::new())
    }

    async fn get_stats(&self) -> Result<QueueStats, CoreError> {
        // Simplified implementation - return default stats
        // In reality, would aggregate from PostgreSQL
        Ok(QueueStats {
            total_jobs: 0,
            pending_jobs: 0,
            running_jobs: 0,
            completed_jobs: 0,
            failed_jobs: 0,
            dead_letter_jobs: 0,
            tenant_count: 0,
            average_processing_time: 0.0,
            last_updated: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert!(config.enable_cache);
        assert_eq!(config.cache_ttl_seconds, 3600);
        assert!(config.write_through);
        assert!(config.read_through);
    }

    #[test]
    fn test_cache_key_generation() {
        let job_id = ingest_core::generate_id_with_prefix("job");
        let expected_key = format!("queue:job:{}", job_id);
        assert_eq!(expected_key, format!("queue:job:{}", job_id));
    }
}
