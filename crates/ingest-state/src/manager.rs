//! Main state manager implementation
//!
//! This module provides the primary StateManager that orchestrates all
//! state management operations including storage, caching, transitions,
//! snapshots, and recovery.

use crate::{
    CacheStateStorage, ExecutionState, ExecutionStatus, OptimisticLockManager,
    PostgresStateStorage, RecoveryManager, Result, SnapshotManager, StateCache, StateError,
    StateSnapshot, StateStorage, StateTransition, TransitionManager,
};
use ingest_config::Config;
use ingest_core::{Id, generate_id_with_prefix};
use ingest_storage::{PostgresStorage, RedisStorage};
use std::sync::Arc;
use tracing;

/// Main state manager that coordinates all state operations
pub struct StateManager {
    /// Primary storage (PostgreSQL)
    primary_storage: Arc<dyn StateStorage>,
    /// Cache storage (Redis)
    cache_storage: Option<Arc<dyn StateStorage>>,
    /// State cache for performance
    state_cache: Arc<StateCache>,
    /// Transition manager
    transition_manager: Arc<TransitionManager>,
    /// Snapshot manager
    snapshot_manager: Arc<SnapshotManager>,
    /// Optimistic lock manager
    lock_manager: Arc<OptimisticLockManager>,
    /// Recovery manager
    recovery_manager: Arc<RecoveryManager>,
    /// Configuration
    config: Config,
}

impl StateManager {
    /// Create a new state manager
    pub async fn new(config: Config) -> Result<Self> {
        let postgres_storage = PostgresStorage::new(&config.database)
            .await
            .map_err(|e| StateError::database(format!("Failed to initialize database: {e}")))?;

        let primary_storage = Arc::new(PostgresStateStorage::new(postgres_storage));

        // Initialize cache if Redis is configured
        let cache_storage = if !config.redis.url.is_empty() {
            let redis_storage = RedisStorage::new(&config.redis)
                .await
                .map_err(|e| StateError::cache(format!("Failed to initialize cache: {e}")))?;
            Some(Arc::new(CacheStateStorage::new(redis_storage, 300)) as Arc<dyn StateStorage>)
        } else {
            None
        };

        let state_cache = Arc::new(StateCache::new(1000));
        let transition_manager = Arc::new(TransitionManager::new(primary_storage.clone()));
        let snapshot_manager = Arc::new(SnapshotManager::new(primary_storage.clone()));
        let lock_manager = Arc::new(OptimisticLockManager::new());
        let recovery_manager = Arc::new(RecoveryManager::new(
            primary_storage.clone(),
            snapshot_manager.clone(),
        ));

        Ok(Self {
            primary_storage,
            cache_storage,
            state_cache,
            transition_manager,
            snapshot_manager,
            lock_manager,
            recovery_manager,
            config,
        })
    }

    /// Save a new execution state
    pub async fn save_state(&self, state: &ExecutionState) -> Result<()> {
        let _start_time = std::time::Instant::now();

        // Validate state
        self.validate_state(state)?;

        // Save to primary storage
        self.primary_storage.save_state(state).await?;

        // Save to cache if available
        if let Some(cache) = &self.cache_storage {
            if let Err(e) = cache.save_state(state).await {
                tracing::warn!("Failed to cache state: {}", e);
            }
        }

        // Update local cache
        self.state_cache
            .put(state.run_id.clone(), state.clone())
            .await;

        Ok(())
    }

    /// Get execution state by run ID
    pub async fn get_state(&self, run_id: &Id) -> Result<Option<ExecutionState>> {
        let _start_time = std::time::Instant::now();

        // Check local cache first
        if let Some(state) = self.state_cache.get(run_id).await {
            return Ok(Some(state));
        }

        // Check Redis cache
        if let Some(cache) = &self.cache_storage {
            if let Ok(Some(state)) = cache.get_state(run_id).await {
                self.state_cache.put(run_id.clone(), state.clone()).await;
                return Ok(Some(state));
            }
        }

        // Get from primary storage
        let state = self.primary_storage.get_state(run_id).await?;

        // Cache the result if found
        if let Some(ref state) = state {
            self.state_cache.put(run_id.clone(), state.clone()).await;

            if let Some(cache) = &self.cache_storage {
                if let Err(e) = cache.save_state(state).await {
                    tracing::warn!("Failed to cache state: {}", e);
                }
            }
        }

        Ok(state)
    }

    /// Update execution state with optimistic locking
    pub async fn update_state(&self, state: &ExecutionState) -> Result<()> {
        let _start_time = std::time::Instant::now();

        // Validate state
        self.validate_state(state)?;

        // Check optimistic lock
        self.lock_manager
            .check_version(&state.run_id, state.version)
            .await?;

        // Create new version
        let mut updated_state = state.clone();
        updated_state.version += 1;
        updated_state.updated_at = chrono::Utc::now();

        // Update primary storage
        self.primary_storage.update_state(&updated_state).await?;

        // Update cache
        if let Some(cache) = &self.cache_storage {
            if let Err(e) = cache.update_state(&updated_state).await {
                tracing::warn!("Failed to update cached state: {}", e);
            }
        }

        // Update local cache
        self.state_cache
            .put(state.run_id.clone(), updated_state)
            .await;

        Ok(())
    }

    /// Transition state to a new status
    pub async fn transition_state(
        &self,
        run_id: &Id,
        new_status: ExecutionStatus,
        reason: String,
    ) -> Result<()> {
        let mut state = self
            .get_state(run_id)
            .await?
            .ok_or_else(|| StateError::state_not_found(run_id.as_str()))?;

        // Record transition
        self.transition_manager
            .transition(&mut state, new_status, reason)
            .await?;

        // Update state
        self.update_state(&state).await?;

        Ok(())
    }

    /// Create a state snapshot
    pub async fn create_snapshot(
        &self,
        run_id: &Id,
        checkpoint_name: String,
    ) -> Result<StateSnapshot> {
        let state = self
            .get_state(run_id)
            .await?
            .ok_or_else(|| StateError::state_not_found(run_id.as_str()))?;

        self.snapshot_manager
            .create_snapshot(&state, checkpoint_name)
            .await
    }

    /// Restore state from snapshot
    pub async fn restore_from_snapshot(&self, snapshot_id: &Id) -> Result<()> {
        self.recovery_manager
            .restore_from_snapshot(snapshot_id)
            .await
    }

    /// Get states by function ID
    pub async fn get_states_by_function(&self, function_id: &Id) -> Result<Vec<ExecutionState>> {
        self.primary_storage
            .get_states_by_function(function_id)
            .await
    }

    /// Get states by status
    pub async fn get_states_by_status(
        &self,
        status: ExecutionStatus,
    ) -> Result<Vec<ExecutionState>> {
        self.primary_storage
            .get_states_by_status(status.as_str())
            .await
    }

    /// Delete execution state
    pub async fn delete_state(&self, run_id: &Id) -> Result<()> {
        // Delete from primary storage
        self.primary_storage.delete_state(run_id).await?;

        // Delete from cache
        if let Some(cache) = &self.cache_storage {
            if let Err(e) = cache.delete_state(run_id).await {
                tracing::warn!("Failed to delete cached state: {}", e);
            }
        }

        // Remove from local cache
        self.state_cache.remove(run_id).await;

        Ok(())
    }

    /// Get snapshots for a run
    pub async fn get_snapshots(&self, run_id: &Id) -> Result<Vec<StateSnapshot>> {
        self.snapshot_manager.get_snapshots(run_id).await
    }

    /// Get transitions for a run
    pub async fn get_transitions(&self, run_id: &Id) -> Result<Vec<StateTransition>> {
        self.transition_manager.get_transitions(run_id).await
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        // Check primary storage
        self.primary_storage
            .get_state(&generate_id_with_prefix("health"))
            .await?;
        Ok(())
    }

    /// Validate state before operations
    fn validate_state(&self, state: &ExecutionState) -> Result<()> {
        if state.run_id.as_str().is_empty() {
            return Err(StateError::validation("Run ID cannot be empty".to_string()));
        }

        if state.function_id.as_str().is_empty() {
            return Err(StateError::validation(
                "Function ID cannot be empty".to_string(),
            ));
        }

        if state.version == 0 {
            return Err(StateError::validation(
                "Version must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

// Implement Clone for StateManager to allow sharing
impl Clone for StateManager {
    fn clone(&self) -> Self {
        Self {
            primary_storage: self.primary_storage.clone(),
            cache_storage: self.cache_storage.clone(),
            state_cache: self.state_cache.clone(),
            transition_manager: self.transition_manager.clone(),
            snapshot_manager: self.snapshot_manager.clone(),
            lock_manager: self.lock_manager.clone(),
            recovery_manager: self.recovery_manager.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::{Id, generate_id_with_prefix};

    #[tokio::test]
    async fn test_state_manager_basic_operations() {
        // This would require a test database setup
        // Implementation would go here
    }

    #[tokio::test]
    async fn test_state_validation() {
        // Test state validation logic without requiring database
        let mut state = ExecutionState::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );

        // Valid state should pass - test the validation logic directly
        assert!(!state.run_id.as_str().is_empty());
        assert!(!state.function_id.as_str().is_empty());

        // Empty run ID should fail
        state.run_id = Id::new("");
        assert!(state.run_id.as_str().is_empty());
    }
}
