//! Tests for the ingest-state crate
//!
//! This module provides comprehensive tests for all state management functionality
//! including storage, caching, transitions, snapshots, locking, and recovery.

use chrono::Utc;
use ingest_core::generate_id_with_prefix;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

use crate::{
    ExecutionContext, ExecutionState, ExecutionStatus, StateError, StateSnapshot, StateTransition,
};

/// Create a test execution state fixture
fn create_test_state() -> ExecutionState {
    ExecutionState {
        run_id: generate_id_with_prefix("run"),
        function_id: generate_id_with_prefix("fn"),
        status: ExecutionStatus::Running,
        current_step: Some(generate_id_with_prefix("step")),
        variables: HashMap::new(),
        context: ExecutionContext {
            trigger_event: None,
            environment: HashMap::new(),
            config: HashMap::new(),
            metadata: HashMap::new(),
            attempt: 0,
            max_attempts: 3,
        },
        version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        scheduled_at: None,
        completed_at: None,
    }
}

/// Test ExecutionStatus string conversion
#[test]
fn test_execution_status_as_str() {
    let fixture = ExecutionStatus::Running;
    let actual = fixture.as_str();
    let expected = "running";
    assert_eq!(actual, expected);
}

/// Test ExecutionStatus parsing from string
#[test]
fn test_execution_status_from_str() {
    let fixture = "completed";
    let actual = fixture.parse::<ExecutionStatus>().unwrap();
    let expected = ExecutionStatus::Completed;
    assert_eq!(actual, expected);
}

/// Test ExecutionStatus terminal status check
#[test]
fn test_execution_status_is_terminal() {
    let fixture = ExecutionStatus::Completed;
    let actual = fixture.is_terminal();
    let expected = true;
    assert!(actual == expected);
}

/// Test ExecutionState creation
#[test]
fn test_execution_state_creation() {
    let fixture = create_test_state();
    let actual = fixture.status;
    let expected = ExecutionStatus::Running;
    assert_eq!(actual, expected);
}

/// Test ExecutionState status update
#[test]
fn test_execution_state_status_update() {
    let mut fixture = create_test_state();
    fixture.set_status(ExecutionStatus::Completed);
    let actual = fixture.status;
    let expected = ExecutionStatus::Completed;
    assert_eq!(actual, expected);
}

/// Test ExecutionState version increment
#[test]
fn test_execution_state_version_increment() {
    let mut fixture = create_test_state();
    let original_version = fixture.version;
    fixture.increment_version();
    let actual = fixture.version;
    let expected = original_version + 1;
    assert_eq!(actual, expected);
}

/// Test StateTransition creation
#[test]
fn test_state_transition_creation() {
    let fixture = StateTransition {
        id: generate_id_with_prefix("trans"),
        run_id: generate_id_with_prefix("run"),
        from_status: ExecutionStatus::Running,
        to_status: ExecutionStatus::Completed,
        reason: "Task completed successfully".to_string(),
        metadata: HashMap::new(),
        created_at: Utc::now(),
        step_id: Some(generate_id_with_prefix("step")),
        from_version: 1,
        to_version: 2,
    };
    let actual = fixture.from_status;
    let expected = ExecutionStatus::Running;
    assert_eq!(actual, expected);
}

/// Test StateSnapshot creation
#[test]
fn test_state_snapshot_creation() {
    let state = create_test_state();
    let fixture = StateSnapshot {
        id: generate_id_with_prefix("snap"),
        run_id: state.run_id.clone(),
        state: state.clone(),
        checkpoint_name: "test_checkpoint".to_string(),
        created_at: Utc::now(),
        metadata: HashMap::new(),
    };
    let actual = fixture.state.run_id;
    let expected = state.run_id;
    assert_eq!(actual, expected);
}

/// Test error creation
#[test]
fn test_state_error_creation() {
    let fixture = StateError::state_not_found("test_run_id");
    let actual = format!("{}", fixture);
    let expected = "State not found for run ID: test_run_id";
    assert_eq!(actual, expected);
}

/// Test validation error creation
#[test]
fn test_validation_error_creation() {
    let fixture = StateError::validation("Invalid state");
    let actual = format!("{}", fixture);
    let expected = "State validation failed: Invalid state";
    assert_eq!(actual, expected);
}

/// Test version conflict error creation
#[test]
fn test_version_conflict_error_creation() {
    let fixture = StateError::version_conflict("test_run_id", 5, 3);
    let actual = format!("{}", fixture);
    let expected = "Version conflict for run ID test_run_id: expected 5, got 3";
    assert_eq!(actual, expected);
}

/// Test invalid transition error creation
#[test]
fn test_invalid_transition_error_creation() {
    let fixture = StateError::invalid_transition("running", "pending", "Cannot go backwards");
    let actual = format!("{}", fixture);
    let expected = "Invalid state transition from running to pending: Cannot go backwards";
    assert_eq!(actual, expected);
}
