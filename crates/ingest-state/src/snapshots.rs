//! Snapshot management for point-in-time state recovery
//!
//! This module provides functionality to create, manage, and restore
//! state snapshots for recovery purposes.

use crate::{ExecutionState, Result, StateError, StateSnapshot, StateStorage};
use ingest_core::{Id, generate_id_with_prefix};
use std::sync::Arc;

/// Manages state snapshots for recovery
pub struct SnapshotManager {
    storage: Arc<dyn StateStorage>,
}

impl SnapshotManager {
    pub fn new(storage: Arc<dyn StateStorage>) -> Self {
        Self { storage }
    }

    /// Create a snapshot of the current state
    pub async fn create_snapshot(
        &self,
        state: &ExecutionState,
        checkpoint_name: String,
    ) -> Result<StateSnapshot> {
        let snapshot = StateSnapshot {
            id: generate_id_with_prefix("snap"),
            run_id: state.run_id.clone(),
            state: state.clone(),
            checkpoint_name,
            metadata: std::collections::HashMap::new(),
            created_at: chrono::Utc::now(),
        };

        self.storage.save_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    /// Create an automatic snapshot at key transition points
    pub async fn create_auto_snapshot(
        &self,
        state: &ExecutionState,
        step_name: &str,
    ) -> Result<StateSnapshot> {
        let checkpoint_name = format!("auto_{}_{}", step_name, chrono::Utc::now().timestamp());
        self.create_snapshot(state, checkpoint_name).await
    }

    /// Get all snapshots for a run
    pub async fn get_snapshots(&self, run_id: &Id) -> Result<Vec<StateSnapshot>> {
        self.storage.get_snapshots(run_id).await
    }

    /// Get a specific snapshot by ID
    pub async fn get_snapshot(&self, _snapshot_id: &Id) -> Result<Option<StateSnapshot>> {
        // This would require a method to get snapshot by ID
        // For now, we'll get all snapshots and filter
        // In a real implementation, we'd add this to the storage trait
        Err(StateError::not_implemented(
            "get_snapshot by ID not implemented".to_string(),
        ))
    }

    /// Get the latest snapshot for a run
    pub async fn get_latest_snapshot(&self, run_id: &Id) -> Result<Option<StateSnapshot>> {
        let mut snapshots = self.get_snapshots(run_id).await?;
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snapshots.into_iter().next())
    }

    /// Get snapshot by checkpoint name
    pub async fn get_snapshot_by_checkpoint(
        &self,
        run_id: &Id,
        checkpoint_name: &str,
    ) -> Result<Option<StateSnapshot>> {
        let snapshots = self.get_snapshots(run_id).await?;
        Ok(snapshots
            .into_iter()
            .find(|s| s.checkpoint_name == checkpoint_name))
    }

    /// Delete old snapshots based on retention policy
    pub async fn cleanup_snapshots(&self, run_id: &Id, keep_count: usize) -> Result<()> {
        let mut snapshots = self.get_snapshots(run_id).await?;

        if snapshots.len() <= keep_count {
            return Ok(()); // Nothing to clean up
        }

        // Sort by creation time, newest first
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Keep the newest ones, delete the rest
        let to_delete = snapshots.into_iter().skip(keep_count);

        for snapshot in to_delete {
            // This would require a delete_snapshot method in storage
            // For now, we'll just log it
            tracing::info!("Would delete snapshot {} for cleanup", snapshot.id);
        }

        Ok(())
    }

    /// Validate snapshot integrity
    pub fn validate_snapshot(&self, snapshot: &StateSnapshot) -> Result<()> {
        if snapshot.id.as_str().is_empty() {
            return Err(StateError::validation(
                "Snapshot ID cannot be empty".to_string(),
            ));
        }

        if snapshot.run_id.as_str().is_empty() {
            return Err(StateError::validation("Run ID cannot be empty".to_string()));
        }

        if snapshot.checkpoint_name.is_empty() {
            return Err(StateError::validation(
                "Checkpoint name cannot be empty".to_string(),
            ));
        }

        // Validate the state within the snapshot
        if snapshot.state.run_id != snapshot.run_id {
            return Err(StateError::validation(
                "Snapshot run ID does not match state run ID".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StateTransition;
    use crate::types::{ExecutionContext, ExecutionStatus};
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_snapshot_creation() {
        let storage = Arc::new(MockStorage::new());
        let manager = SnapshotManager::new(storage);

        let state = ExecutionState {
            run_id: generate_id_with_prefix("run"),
            function_id: generate_id_with_prefix("fn"),
            status: ExecutionStatus::Running,
            current_step: None,
            variables: HashMap::new(),
            context: ExecutionContext::default(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            scheduled_at: None,
            completed_at: None,
        };

        let snapshot = manager
            .create_snapshot(&state, "test_checkpoint".to_string())
            .await
            .unwrap();

        assert_eq!(snapshot.run_id, state.run_id);
        assert_eq!(snapshot.checkpoint_name, "test_checkpoint");
        assert_eq!(snapshot.state.status, ExecutionStatus::Running);
    }

    #[test]
    fn test_snapshot_validation() {
        let storage = Arc::new(MockStorage::new());
        let manager = SnapshotManager::new(storage);

        let run_id = generate_id_with_prefix("run");
        let state = ExecutionState {
            run_id: run_id.clone(),
            function_id: generate_id_with_prefix("fn"),
            status: ExecutionStatus::Running,
            current_step: None,
            variables: HashMap::new(),
            context: ExecutionContext::default(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            scheduled_at: None,
            completed_at: None,
        };

        let snapshot = StateSnapshot {
            id: generate_id_with_prefix("snap"),
            run_id: run_id.clone(),
            state,
            checkpoint_name: "test".to_string(),
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        };

        // Valid snapshot should pass
        assert!(manager.validate_snapshot(&snapshot).is_ok());

        // Invalid snapshot with mismatched run ID should fail
        let mut invalid_snapshot = snapshot.clone();
        invalid_snapshot.state.run_id = generate_id_with_prefix("different");
        assert!(manager.validate_snapshot(&invalid_snapshot).is_err());
    }

    // Mock storage for testing
    struct MockStorage {
        snapshots: std::sync::Mutex<Vec<StateSnapshot>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                snapshots: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl StateStorage for MockStorage {
        async fn save_state(&self, _state: &ExecutionState) -> Result<()> {
            Ok(())
        }
        async fn get_state(&self, _run_id: &Id) -> Result<Option<ExecutionState>> {
            Ok(None)
        }
        async fn update_state(&self, _state: &ExecutionState) -> Result<()> {
            Ok(())
        }
        async fn delete_state(&self, _run_id: &Id) -> Result<()> {
            Ok(())
        }
        async fn get_states_by_function(&self, _function_id: &Id) -> Result<Vec<ExecutionState>> {
            Ok(vec![])
        }
        async fn get_states_by_status(&self, _status: &str) -> Result<Vec<ExecutionState>> {
            Ok(vec![])
        }

        async fn save_snapshot(&self, snapshot: &StateSnapshot) -> Result<()> {
            let mut snapshots = self.snapshots.lock().unwrap();
            snapshots.push(snapshot.clone());
            Ok(())
        }

        async fn get_snapshots(&self, run_id: &Id) -> Result<Vec<StateSnapshot>> {
            let snapshots = self.snapshots.lock().unwrap();
            Ok(snapshots
                .iter()
                .filter(|s| &s.run_id == run_id)
                .cloned()
                .collect())
        }

        async fn save_transition(&self, _transition: &StateTransition) -> Result<()> {
            Ok(())
        }
        async fn get_transitions(&self, _run_id: &Id) -> Result<Vec<StateTransition>> {
            Ok(vec![])
        }
    }
}
