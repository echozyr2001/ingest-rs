//! Recovery and rollback functionality
//!
//! This module provides recovery mechanisms for state corruption,
//! rollback capabilities, and disaster recovery features.

use crate::{ExecutionState, Result, SnapshotManager, StateError, StateSnapshot, StateStorage};
use ingest_core::{DateTime, Id};
use std::sync::Arc;

/// Manages state recovery and rollback operations
pub struct RecoveryManager {
    storage: Arc<dyn StateStorage>,
    snapshot_manager: Arc<SnapshotManager>,
}

impl RecoveryManager {
    pub fn new(storage: Arc<dyn StateStorage>, snapshot_manager: Arc<SnapshotManager>) -> Self {
        Self {
            storage,
            snapshot_manager,
        }
    }

    /// Restore state from a specific snapshot
    pub async fn restore_from_snapshot(&self, _snapshot_id: &Id) -> Result<()> {
        // This would require getting snapshot by ID from storage
        // For now, we'll return not implemented
        Err(StateError::not_implemented(
            "restore_from_snapshot requires get_snapshot_by_id implementation".to_string(),
        ))
    }

    /// Restore state from the latest snapshot
    pub async fn restore_from_latest_snapshot(&self, run_id: &Id) -> Result<()> {
        let snapshot = self
            .snapshot_manager
            .get_latest_snapshot(run_id)
            .await?
            .ok_or_else(|| {
                StateError::snapshot_not_found(format!("No snapshots found for run {run_id}"))
            })?;

        // Validate snapshot before restoration
        self.snapshot_manager.validate_snapshot(&snapshot)?;

        // Restore the state
        let mut restored_state = snapshot.state.clone();
        restored_state.version += 1; // Increment version for the restoration
        restored_state.updated_at = chrono::Utc::now();

        // Update the state in storage
        self.storage.update_state(&restored_state).await?;

        tracing::info!(
            "Restored state for run {} from snapshot {} (checkpoint: {})",
            run_id,
            snapshot.id,
            snapshot.checkpoint_name
        );

        Ok(())
    }

    /// Restore state from a named checkpoint
    pub async fn restore_from_checkpoint(&self, run_id: &Id, checkpoint_name: &str) -> Result<()> {
        let snapshot = self
            .snapshot_manager
            .get_snapshot_by_checkpoint(run_id, checkpoint_name)
            .await?
            .ok_or_else(|| {
                StateError::snapshot_not_found(format!(
                    "Checkpoint '{checkpoint_name}' not found for run {run_id}"
                ))
            })?;

        // Validate snapshot before restoration
        self.snapshot_manager.validate_snapshot(&snapshot)?;

        // Restore the state
        let mut restored_state = snapshot.state.clone();
        restored_state.version += 1;
        restored_state.updated_at = chrono::Utc::now();

        self.storage.update_state(&restored_state).await?;

        tracing::info!(
            "Restored state for run {} from checkpoint '{}' (snapshot: {})",
            run_id,
            checkpoint_name,
            snapshot.id
        );

        Ok(())
    }

    /// Rollback state to a previous version
    pub async fn rollback_to_version(&self, run_id: &Id, target_version: u64) -> Result<()> {
        // Get all snapshots for the run
        let snapshots = self.snapshot_manager.get_snapshots(run_id).await?;

        // Find snapshot with the target version or closest lower version
        let target_snapshot = snapshots
            .into_iter()
            .filter(|s| s.state.version <= target_version)
            .max_by_key(|s| s.state.version)
            .ok_or_else(|| {
                StateError::snapshot_not_found(format!(
                    "No snapshot found for version {target_version} or lower"
                ))
            })?;

        // Restore from the found snapshot
        let mut restored_state = target_snapshot.state.clone();
        restored_state.version = target_version + 1; // Set to next version
        restored_state.updated_at = chrono::Utc::now();

        self.storage.update_state(&restored_state).await?;

        tracing::info!(
            "Rolled back state for run {} to version {} (from snapshot {})",
            run_id,
            target_version,
            target_snapshot.id
        );

        Ok(())
    }

    /// Detect and repair state corruption
    pub async fn detect_and_repair_corruption(&self, run_id: &Id) -> Result<bool> {
        let current_state = self.storage.get_state(run_id).await?.ok_or_else(|| {
            StateError::state_not_found(format!("State not found for run {run_id}"))
        })?;

        // Check for corruption indicators
        let corruption_detected = self.detect_corruption(&current_state).await?;

        if corruption_detected {
            tracing::warn!("Corruption detected in state for run {}", run_id);

            // Attempt to repair from latest snapshot
            match self.restore_from_latest_snapshot(run_id).await {
                Ok(()) => {
                    tracing::info!("Successfully repaired corrupted state for run {}", run_id);
                    Ok(true)
                }
                Err(e) => {
                    tracing::error!("Failed to repair corrupted state for run {}: {}", run_id, e);
                    Err(e)
                }
            }
        } else {
            Ok(false) // No corruption detected
        }
    }

    /// Detect corruption in state
    async fn detect_corruption(&self, state: &ExecutionState) -> Result<bool> {
        // Basic corruption checks

        // Check for invalid version
        if state.version == 0 {
            return Ok(true);
        }

        // Check for invalid timestamps
        if state.updated_at < state.created_at {
            return Ok(true);
        }

        // Check for completed state without completion time
        if state.status.is_terminal() && state.completed_at.is_none() {
            return Ok(true);
        }

        // Check for non-terminal state with completion time
        if !state.status.is_terminal() && state.completed_at.is_some() {
            return Ok(true);
        }

        // Additional corruption checks could be added here
        // - Validate JSON structure in variables and context
        // - Check for orphaned references
        // - Verify state consistency with transitions

        Ok(false) // No corruption detected
    }

    /// Create a recovery point before risky operations
    pub async fn create_recovery_point(
        &self,
        run_id: &Id,
        operation_name: &str,
    ) -> Result<StateSnapshot> {
        let state = self.storage.get_state(run_id).await?.ok_or_else(|| {
            StateError::state_not_found(format!("State not found for run {run_id}"))
        })?;

        let checkpoint_name = format!("recovery_point_{operation_name}");
        self.snapshot_manager
            .create_snapshot(&state, checkpoint_name)
            .await
    }

    /// Validate state integrity
    pub async fn validate_state_integrity(&self, run_id: &Id) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        let state = match self.storage.get_state(run_id).await? {
            Some(state) => state,
            None => {
                issues.push("State not found".to_string());
                return Ok(issues);
            }
        };

        // Check basic state validity
        if state.run_id.as_str().is_empty() {
            issues.push("Empty run ID".to_string());
        }

        if state.function_id.as_str().is_empty() {
            issues.push("Empty function ID".to_string());
        }

        if state.version == 0 {
            issues.push("Invalid version (0)".to_string());
        }

        if state.updated_at < state.created_at {
            issues.push("Updated time is before created time".to_string());
        }

        // Check status consistency
        if state.status.is_terminal() && state.completed_at.is_none() {
            issues.push("Terminal status without completion time".to_string());
        }

        if !state.status.is_terminal() && state.completed_at.is_some() {
            issues.push("Non-terminal status with completion time".to_string());
        }

        // Check transitions consistency
        let transitions = self.storage.get_transitions(run_id).await?;
        if !transitions.is_empty() {
            let last_transition = transitions.last().unwrap();
            if last_transition.to_status != state.status {
                issues.push(format!(
                    "State status ({}) doesn't match last transition ({})",
                    state.status.as_str(),
                    last_transition.to_status.as_str()
                ));
            }
        }

        Ok(issues)
    }

    /// Get recovery statistics
    pub async fn get_recovery_stats(&self, run_id: &Id) -> Result<RecoveryStats> {
        let snapshots = self.snapshot_manager.get_snapshots(run_id).await?;
        let transitions = self.storage.get_transitions(run_id).await?;

        let snapshot_count = snapshots.len();
        let transition_count = transitions.len();

        let latest_snapshot = snapshots.first().map(|s| s.created_at);
        let latest_transition = transitions.last().map(|t| t.created_at);

        Ok(RecoveryStats {
            snapshot_count,
            transition_count,
            latest_snapshot,
            latest_transition,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub snapshot_count: usize,
    pub transition_count: usize,
    pub latest_snapshot: Option<DateTime>,
    pub latest_transition: Option<DateTime>,
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
    async fn test_corruption_detection() {
        let storage = Arc::new(MockStorage::new());
        let snapshot_manager = Arc::new(SnapshotManager::new(storage.clone()));
        let recovery_manager = RecoveryManager::new(storage, snapshot_manager);

        // Valid state should not be detected as corrupted
        let valid_state = ExecutionState {
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

        assert!(
            !recovery_manager
                .detect_corruption(&valid_state)
                .await
                .unwrap()
        );

        // State with version 0 should be detected as corrupted
        let mut corrupted_state = valid_state.clone();
        corrupted_state.version = 0;
        assert!(
            recovery_manager
                .detect_corruption(&corrupted_state)
                .await
                .unwrap()
        );

        // State with updated_at before created_at should be corrupted
        let mut corrupted_state = valid_state.clone();
        corrupted_state.updated_at = corrupted_state.created_at - chrono::Duration::hours(1);
        assert!(
            recovery_manager
                .detect_corruption(&corrupted_state)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_state_integrity_validation() {
        let storage = Arc::new(MockStorage::new());
        let snapshot_manager = Arc::new(SnapshotManager::new(storage.clone()));
        let recovery_manager = RecoveryManager::new(storage, snapshot_manager);

        let run_id = generate_id_with_prefix("run");

        // Test with non-existent state
        let issues = recovery_manager
            .validate_state_integrity(&run_id)
            .await
            .unwrap();
        assert_eq!(issues, vec!["State not found"]);
    }

    // Mock storage for testing
    struct MockStorage;

    impl MockStorage {
        fn new() -> Self {
            Self
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
        async fn save_snapshot(&self, _snapshot: &StateSnapshot) -> Result<()> {
            Ok(())
        }
        async fn get_snapshots(&self, _run_id: &Id) -> Result<Vec<StateSnapshot>> {
            Ok(vec![])
        }
        async fn save_transition(&self, _transition: &StateTransition) -> Result<()> {
            Ok(())
        }
        async fn get_transitions(&self, _run_id: &Id) -> Result<Vec<StateTransition>> {
            Ok(vec![])
        }
    }
}
