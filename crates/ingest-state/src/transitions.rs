//! State transition management
//!
//! This module handles state transitions with validation, history tracking,
//! and rollback capabilities.

use crate::{ExecutionState, ExecutionStatus, Result, StateError, StateStorage, StateTransition};
use ingest_core::{Id, generate_id_with_prefix};
use std::collections::HashMap;
use std::sync::Arc;

/// Manages state transitions with validation and history
pub struct TransitionManager {
    storage: Arc<dyn StateStorage>,
}

impl TransitionManager {
    pub fn new(storage: Arc<dyn StateStorage>) -> Self {
        Self { storage }
    }

    /// Perform a state transition
    pub async fn transition(
        &self,
        state: &mut ExecutionState,
        new_status: ExecutionStatus,
        reason: String,
    ) -> Result<()> {
        let old_status = state.status.clone();

        // Validate transition
        self.validate_transition(&old_status, &new_status)?;

        // Record transition
        let transition = StateTransition {
            id: generate_id_with_prefix("trans"),
            run_id: state.run_id.clone(),
            from_status: old_status.clone(),
            to_status: new_status.clone(),
            reason,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now(),
            step_id: None,
            from_version: state.version,
            to_version: state.version + 1,
        };

        // Save transition record
        self.storage.save_transition(&transition).await?;

        // Update state
        state.set_status(new_status);

        Ok(())
    }

    /// Get transition history for a run
    pub async fn get_transitions(&self, run_id: &Id) -> Result<Vec<StateTransition>> {
        self.storage.get_transitions(run_id).await
    }

    /// Validate if a transition is allowed
    fn validate_transition(&self, from: &ExecutionStatus, to: &ExecutionStatus) -> Result<()> {
        use ExecutionStatus::*;

        let valid = match (from, to) {
            // Same state is allowed (idempotent)
            (a, b) if a == b => true,

            // From Pending
            (Pending, Running) => true,
            (Pending, Cancelled) => true,
            (Pending, Failed) => true,

            // From Running
            (Running, Completed) => true,
            (Running, Failed) => true,
            (Running, Cancelled) => true,
            (Running, Paused) => true,
            (Running, Sleeping) => true,

            // From Paused
            (Paused, Running) => true,
            (Paused, Cancelled) => true,
            (Paused, Failed) => true,

            // From Sleeping
            (Sleeping, Running) => true,
            (Sleeping, Cancelled) => true,
            (Sleeping, Failed) => true,

            // Terminal states cannot transition to other states
            (Completed, _) => false,
            (Failed, _) => false,
            (Cancelled, _) => false,

            // All other transitions are invalid
            _ => false,
        };

        if !valid {
            return Err(StateError::invalid_transition(
                format!("{from:?}"),
                format!("{to:?}"),
                format!("Cannot transition from {from:?} to {to:?}"),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StateSnapshot;

    #[test]
    fn test_transition_validation() {
        let storage = Arc::new(MockStorage::new());
        let manager = TransitionManager::new(storage);

        // Valid transitions
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Pending, &ExecutionStatus::Running)
                .is_ok()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Running, &ExecutionStatus::Completed)
                .is_ok()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Running, &ExecutionStatus::Paused)
                .is_ok()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Paused, &ExecutionStatus::Running)
                .is_ok()
        );

        // Invalid transitions
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Completed, &ExecutionStatus::Running)
                .is_err()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Failed, &ExecutionStatus::Running)
                .is_err()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Cancelled, &ExecutionStatus::Running)
                .is_err()
        );

        // Idempotent transitions
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Running, &ExecutionStatus::Running)
                .is_ok()
        );
        assert!(
            manager
                .validate_transition(&ExecutionStatus::Completed, &ExecutionStatus::Completed)
                .is_ok()
        );
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
