use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use ingest_core::Error as CoreError;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::traits::{TenantManager, TenantStats};
use crate::types::{QueueJob, TenantId};

/// Simplified tenant manager implementation
pub struct SimpleTenantManager {
    /// Tenant statistics storage
    tenant_stats: DashMap<TenantId, Arc<RwLock<TenantStats>>>,
    /// Round-robin scheduling state
    round_robin: Arc<RwLock<Vec<TenantId>>>,
    /// Current round-robin position
    current_position: Arc<RwLock<usize>>,
    /// Maximum number of tenants allowed
    max_tenants: u32,
    /// Whether quota enforcement is enabled
    quota_enforcement: bool,
}

impl SimpleTenantManager {
    /// Creates a new tenant manager instance
    pub fn new(max_tenants: u32, quota_enforcement: bool) -> Self {
        Self {
            tenant_stats: DashMap::new(),
            round_robin: Arc::new(RwLock::new(Vec::new())),
            current_position: Arc::new(RwLock::new(0)),
            max_tenants,
            quota_enforcement,
        }
    }

    /// Gets or creates tenant statistics for the given tenant ID
    async fn get_or_create_stats(&self, tenant_id: &TenantId) -> Arc<RwLock<TenantStats>> {
        if let Some(stats) = self.tenant_stats.get(tenant_id) {
            stats.clone()
        } else {
            let new_stats = Arc::new(RwLock::new(TenantStats {
                tenant_id: tenant_id.clone(),
                active_jobs: 0,
                completed_jobs: 0,
                failed_jobs: 0,
                quota_used: 0.0,
                quota_limit: 100.0,
                last_activity: Utc::now(),
            }));

            self.tenant_stats
                .insert(tenant_id.clone(), new_stats.clone());

            // Add to round-robin list
            let mut round_robin = self.round_robin.write().await;
            if !round_robin.contains(tenant_id) {
                round_robin.push(tenant_id.clone());
            }

            new_stats
        }
    }

    /// Checks if tenant quota allows the operation
    async fn check_quota(&self, tenant_id: &TenantId) -> Result<bool, CoreError> {
        if !self.quota_enforcement {
            return Ok(true);
        }

        let stats_arc = self.get_or_create_stats(tenant_id).await;
        let stats = stats_arc.read().await;

        Ok(stats.quota_used < stats.quota_limit)
    }

    /// Returns the current number of tenants
    pub fn tenant_count(&self) -> usize {
        self.tenant_stats.len()
    }

    /// Cleans up inactive tenants
    pub async fn cleanup_inactive_tenants(
        &self,
        max_inactive_hours: i64,
    ) -> Result<u64, CoreError> {
        let now = Utc::now();
        let mut cleaned = 0u64;
        let cutoff = now - chrono::Duration::hours(max_inactive_hours);

        let inactive_tenants: Vec<TenantId> = self
            .tenant_stats
            .iter()
            .filter_map(|entry| {
                let tenant_id = entry.key();
                let stats = entry.value();

                // Try to acquire read lock, skip if failed
                if let Ok(stats_guard) = stats.try_read() {
                    if stats_guard.last_activity < cutoff && stats_guard.active_jobs == 0 {
                        Some(tenant_id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for tenant_id in inactive_tenants {
            self.tenant_stats.remove(&tenant_id);

            // Remove from round-robin list
            let mut round_robin = self.round_robin.write().await;
            round_robin.retain(|id| *id != tenant_id);

            cleaned += 1;
        }

        Ok(cleaned)
    }
}

#[async_trait]
impl TenantManager for SimpleTenantManager {
    async fn can_enqueue(&self, tenant_id: &TenantId) -> Result<bool, CoreError> {
        // Check maximum tenant limit
        if self.tenant_count() >= self.max_tenants as usize {
            // Allow existing tenants to continue
            if !self.tenant_stats.contains_key(tenant_id) {
                return Ok(false);
            }
        }

        // Check quota
        self.check_quota(tenant_id).await
    }

    async fn record_job(&self, tenant_id: &TenantId, job: &QueueJob) -> Result<(), CoreError> {
        let stats_arc = self.get_or_create_stats(tenant_id).await;
        let mut stats = stats_arc.write().await;

        match job.status {
            crate::types::JobStatus::Pending => {
                stats.active_jobs += 1;
                stats.quota_used += 1.0;
            }
            crate::types::JobStatus::Active => {
                // Job status changed from pending to active, no count increase
            }
            crate::types::JobStatus::Completed => {
                if stats.active_jobs > 0 {
                    stats.active_jobs -= 1;
                }
                stats.completed_jobs += 1;
            }
            crate::types::JobStatus::Failed => {
                if stats.active_jobs > 0 {
                    stats.active_jobs -= 1;
                }
                stats.failed_jobs += 1;
            }
            crate::types::JobStatus::DeadLetter | crate::types::JobStatus::Scheduled => {
                // Other status transitions not handled
            }
        }

        stats.last_activity = Utc::now();
        Ok(())
    }

    async fn get_next_tenant(&self) -> Result<Option<TenantId>, CoreError> {
        let round_robin = self.round_robin.read().await;

        if round_robin.is_empty() {
            return Ok(None);
        }

        let mut position = self.current_position.write().await;
        let tenant_id = round_robin.get(*position).cloned();

        // Update position
        *position = (*position + 1) % round_robin.len();

        Ok(tenant_id)
    }

    async fn get_tenant_stats(&self, tenant_id: &TenantId) -> Result<TenantStats, CoreError> {
        let stats_arc = self.get_or_create_stats(tenant_id).await;
        let stats = stats_arc.read().await;
        Ok(stats.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{JobPriority, JobStatus};
    use pretty_assertions::assert_eq;

    fn create_test_job(tenant_id: TenantId, status: JobStatus) -> QueueJob {
        QueueJob {
            id: ingest_core::generate_id_with_prefix("job"),
            tenant_id,
            function_id: ingest_core::generate_id_with_prefix("fn"),
            payload: serde_json::json!({"test": "data"}),
            priority: JobPriority::Normal,
            status,
            retry_count: 0,
            max_retries: 3,
            created_at: Utc::now(),
            scheduled_for: Utc::now(),
            timeout: std::time::Duration::from_secs(300),
            metadata: crate::types::JobMetadata::new(),
            last_error: None,
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_simple_tenant_manager_creation() {
        let manager = SimpleTenantManager::new(100, true);

        assert_eq!(manager.tenant_count(), 0);
        assert_eq!(manager.quota_enforcement, true);
        assert_eq!(manager.max_tenants, 100);
    }

    #[tokio::test]
    async fn test_can_enqueue_new_tenant() {
        let manager = SimpleTenantManager::new(100, true);
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");

        let result = manager.can_enqueue(&tenant_id).await.unwrap();
        assert_eq!(result, true);
    }

    #[tokio::test]
    async fn test_record_job_stats() {
        let manager = SimpleTenantManager::new(100, true);
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");
        let job = create_test_job(tenant_id.clone(), JobStatus::Pending);

        manager.record_job(&tenant_id, &job).await.unwrap();

        let stats = manager.get_tenant_stats(&tenant_id).await.unwrap();
        assert_eq!(stats.active_jobs, 1);
        assert_eq!(stats.quota_used, 1.0);
    }

    #[tokio::test]
    async fn test_job_completion_updates_stats() {
        let manager = SimpleTenantManager::new(100, true);
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");

        // Record pending job
        let pending_job = create_test_job(tenant_id.clone(), JobStatus::Pending);
        manager.record_job(&tenant_id, &pending_job).await.unwrap();

        // Record completed job
        let completed_job = create_test_job(tenant_id.clone(), JobStatus::Completed);
        manager
            .record_job(&tenant_id, &completed_job)
            .await
            .unwrap();

        let stats = manager.get_tenant_stats(&tenant_id).await.unwrap();
        assert_eq!(stats.active_jobs, 0); // Decreased by 1
        assert_eq!(stats.completed_jobs, 1); // Increased by 1
    }

    #[tokio::test]
    async fn test_round_robin_tenant_selection() {
        let manager = SimpleTenantManager::new(100, true);
        let tenant1 = ingest_core::generate_id_with_prefix("tenant");
        let tenant2 = ingest_core::generate_id_with_prefix("tenant");

        // Create statistics for two tenants
        manager.get_or_create_stats(&tenant1).await;
        manager.get_or_create_stats(&tenant2).await;

        // Next tenant should be round-robin
        let first = manager.get_next_tenant().await.unwrap();
        let second = manager.get_next_tenant().await.unwrap();
        let third = manager.get_next_tenant().await.unwrap();

        assert!(first.is_some());
        assert!(second.is_some());
        assert_eq!(first, third); // Should return to first
    }

    #[tokio::test]
    async fn test_max_tenants_limit() {
        let manager = SimpleTenantManager::new(2, true); // Max 2 tenants
        let tenant1 = ingest_core::generate_id_with_prefix("tenant");
        let tenant2 = ingest_core::generate_id_with_prefix("tenant");
        let tenant3 = ingest_core::generate_id_with_prefix("tenant");

        // First two tenants should be accepted
        assert!(manager.can_enqueue(&tenant1).await.unwrap());
        manager.get_or_create_stats(&tenant1).await;

        assert!(manager.can_enqueue(&tenant2).await.unwrap());
        manager.get_or_create_stats(&tenant2).await;

        // Third tenant should be rejected
        assert!(!manager.can_enqueue(&tenant3).await.unwrap());
    }

    #[tokio::test]
    async fn test_quota_enforcement_disabled() {
        let manager = SimpleTenantManager::new(100, false); // Quota enforcement disabled
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");

        // Should allow even when quota exceeded
        let result = manager.can_enqueue(&tenant_id).await.unwrap();
        assert_eq!(result, true);
    }

    #[tokio::test]
    async fn test_cleanup_inactive_tenants() {
        let manager = SimpleTenantManager::new(100, true);
        let tenant_id = ingest_core::generate_id_with_prefix("tenant");

        // Create a tenant
        let stats_arc = manager.get_or_create_stats(&tenant_id).await;
        {
            let mut stats = stats_arc.write().await;
            // Set to very old activity time
            stats.last_activity = Utc::now() - chrono::Duration::hours(48);
            stats.active_jobs = 0; // No active jobs
        }

        assert_eq!(manager.tenant_count(), 1);

        // Clean up inactive tenants (over 24 hours inactive)
        let cleaned = manager.cleanup_inactive_tenants(24).await.unwrap();

        assert_eq!(cleaned, 1);
        assert_eq!(manager.tenant_count(), 0);
    }
}
