use derive_setters::Setters;
use ingest_core::{DateTime, Id, Json};
use ingest_core::{FunctionId, StepId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Execution status for a function run
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution is currently running
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
    /// Execution is paused/waiting
    Paused,
    /// Execution is sleeping/delayed
    Sleeping,
}

impl ExecutionStatus {
    /// Convert status to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Paused => "paused",
            Self::Sleeping => "sleeping",
        }
    }

    /// Check if status is terminal (cannot change)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Check if status is active (can continue execution)
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused | Self::Sleeping)
    }

    /// Check if status indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed)
    }

    /// Check if status indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled)
    }
}

impl FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "paused" => Ok(Self::Paused),
            "sleeping" => Ok(Self::Sleeping),
            _ => Err(format!("Invalid execution status: {s}")),
        }
    }
}

/// Execution context containing runtime information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ExecutionContext {
    /// Event that triggered this execution
    pub trigger_event: Option<Json>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Runtime configuration
    pub config: HashMap<String, Json>,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
    /// Attempt number for retries
    pub attempt: u32,
    /// Maximum retry attempts
    pub max_attempts: u32,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            trigger_event: None,
            environment: HashMap::new(),
            config: HashMap::new(),
            metadata: HashMap::new(),
            attempt: 1,
            max_attempts: 3,
        }
    }
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the trigger event
    pub fn with_trigger_event(mut self, event: Json) -> Self {
        self.trigger_event = Some(event);
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Add configuration
    pub fn with_config(mut self, key: impl Into<String>, value: Json) -> Self {
        self.config.insert(key.into(), value);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if can retry
    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_attempts
    }

    /// Increment attempt counter
    pub fn increment_attempt(&mut self) {
        self.attempt += 1;
    }
}

impl ExecutionState {
    /// Increment the version number
    pub fn increment_version(&mut self) {
        self.version += 1;
        self.updated_at = chrono::Utc::now();
    }
}

/// Execution state for a function run
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ExecutionState {
    /// Unique run identifier
    pub run_id: Id,
    /// Function being executed
    pub function_id: FunctionId,
    /// Current execution status
    pub status: ExecutionStatus,
    /// Current step being executed
    pub current_step: Option<StepId>,
    /// State variables
    pub variables: HashMap<String, Json>,
    /// Execution context
    pub context: ExecutionContext,
    /// Version for optimistic locking
    pub version: u64,
    /// State creation timestamp
    pub created_at: DateTime,
    /// Last update timestamp
    pub updated_at: DateTime,
    /// Scheduled execution time
    pub scheduled_at: Option<DateTime>,
    /// Completion timestamp
    pub completed_at: Option<DateTime>,
}

impl ExecutionState {
    /// Create a new execution state
    pub fn new(run_id: Id, function_id: FunctionId) -> Self {
        let now = chrono::Utc::now();
        Self {
            run_id,
            function_id,
            status: ExecutionStatus::Pending,
            current_step: None,
            variables: HashMap::new(),
            context: ExecutionContext::new(),
            version: 1,
            created_at: now,
            updated_at: now,
            scheduled_at: None,
            completed_at: None,
        }
    }

    /// Update the state and increment version
    pub fn update(&mut self) {
        self.version += 1;
        self.updated_at = chrono::Utc::now();
    }

    /// Set execution status
    pub fn set_status(&mut self, status: ExecutionStatus) {
        if status.is_terminal() {
            self.completed_at = Some(chrono::Utc::now());
        }
        self.status = status;
        self.update();
    }

    /// Set current step
    pub fn set_current_step(&mut self, step_id: Option<StepId>) {
        self.current_step = step_id;
        self.update();
    }

    /// Set a variable
    pub fn set_variable(&mut self, key: impl Into<String>, value: Json) {
        self.variables.insert(key.into(), value);
        self.update();
    }

    /// Get a variable
    pub fn get_variable(&self, key: &str) -> Option<&Json> {
        self.variables.get(key)
    }

    /// Remove a variable
    pub fn remove_variable(&mut self, key: &str) -> Option<Json> {
        let result = self.variables.remove(key);
        if result.is_some() {
            self.update();
        }
        result
    }

    /// Check if state is terminal
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Check if state is active
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Get execution duration if completed
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completed_at
            .map(|completed| completed - self.created_at)
    }

    /// Validate the execution state
    pub fn validate(&self) -> crate::Result<()> {
        if self.version == 0 {
            return Err(crate::StateError::validation("Version cannot be zero"));
        }

        if self.updated_at < self.created_at {
            return Err(crate::StateError::validation(
                "Updated timestamp cannot be before created timestamp",
            ));
        }

        Ok(())
    }
}

/// State snapshot for point-in-time recovery
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct StateSnapshot {
    /// Unique snapshot identifier
    pub id: Id,
    /// Run this snapshot belongs to
    pub run_id: Id,
    /// Snapshot of the execution state
    pub state: ExecutionState,
    /// Checkpoint name/description
    pub checkpoint_name: String,
    /// Snapshot creation timestamp
    pub created_at: DateTime,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl StateSnapshot {
    /// Create a new state snapshot
    pub fn new(run_id: Id, state: ExecutionState, checkpoint_name: impl Into<String>) -> Self {
        Self {
            id: ingest_core::generate_id_with_prefix("snap"),
            run_id,
            state,
            checkpoint_name: checkpoint_name.into(),
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the snapshot
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Validate the snapshot
    pub fn validate(&self) -> crate::Result<()> {
        if self.checkpoint_name.is_empty() {
            return Err(crate::StateError::validation(
                "Checkpoint name cannot be empty",
            ));
        }

        if self.checkpoint_name.len() > 255 {
            return Err(crate::StateError::validation(
                "Checkpoint name too long (max 255 characters)",
            ));
        }

        self.state.validate()?;

        Ok(())
    }
}

/// State transition record
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct StateTransition {
    /// Unique transition identifier
    pub id: Id,
    /// Run this transition belongs to
    pub run_id: Id,
    /// Previous status
    pub from_status: ExecutionStatus,
    /// New status
    pub to_status: ExecutionStatus,
    /// Reason for the transition
    pub reason: String,
    /// Transition metadata
    pub metadata: HashMap<String, String>,
    /// Transition timestamp
    pub created_at: DateTime,
    /// Step that triggered the transition
    pub step_id: Option<StepId>,
    /// Version before transition
    pub from_version: u64,
    /// Version after transition
    pub to_version: u64,
}

impl StateTransition {
    /// Create a new state transition
    pub fn new(
        run_id: Id,
        from_status: ExecutionStatus,
        to_status: ExecutionStatus,
        reason: impl Into<String>,
        from_version: u64,
        to_version: u64,
    ) -> Self {
        Self {
            id: ingest_core::generate_id_with_prefix("trans"),
            run_id,
            from_status,
            to_status,
            reason: reason.into(),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now(),
            step_id: None,
            from_version,
            to_version,
        }
    }

    /// Set the step that triggered this transition
    pub fn with_step(mut self, step_id: StepId) -> Self {
        self.step_id = Some(step_id);
        self
    }

    /// Add metadata to the transition
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if this is a valid transition
    pub fn is_valid_transition(&self) -> bool {
        match (&self.from_status, &self.to_status) {
            // Valid transitions from Pending
            (ExecutionStatus::Pending, ExecutionStatus::Running) => true,
            (ExecutionStatus::Pending, ExecutionStatus::Cancelled) => true,

            // Valid transitions from Running
            (ExecutionStatus::Running, ExecutionStatus::Completed) => true,
            (ExecutionStatus::Running, ExecutionStatus::Failed) => true,
            (ExecutionStatus::Running, ExecutionStatus::Paused) => true,
            (ExecutionStatus::Running, ExecutionStatus::Sleeping) => true,
            (ExecutionStatus::Running, ExecutionStatus::Cancelled) => true,

            // Valid transitions from Paused
            (ExecutionStatus::Paused, ExecutionStatus::Running) => true,
            (ExecutionStatus::Paused, ExecutionStatus::Cancelled) => true,

            // Valid transitions from Sleeping
            (ExecutionStatus::Sleeping, ExecutionStatus::Running) => true,
            (ExecutionStatus::Sleeping, ExecutionStatus::Cancelled) => true,

            // Same status (no-op)
            (from, to) if from == to => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Validate the transition
    pub fn validate(&self) -> crate::Result<()> {
        if self.reason.is_empty() {
            return Err(crate::StateError::validation(
                "Transition reason cannot be empty",
            ));
        }

        if !self.is_valid_transition() {
            return Err(crate::StateError::invalid_transition(
                format!("{:?}", self.from_status),
                format!("{:?}", self.to_status),
                "Invalid state transition",
            ));
        }

        if self.to_version <= self.from_version {
            return Err(crate::StateError::validation(
                "To version must be greater than from version",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_execution_status_is_terminal() {
        assert!(ExecutionStatus::Completed.is_terminal());
        assert!(ExecutionStatus::Failed.is_terminal());
        assert!(ExecutionStatus::Cancelled.is_terminal());
        assert!(!ExecutionStatus::Running.is_terminal());
        assert!(!ExecutionStatus::Pending.is_terminal());
    }

    #[test]
    fn test_execution_status_is_active() {
        assert!(ExecutionStatus::Running.is_active());
        assert!(ExecutionStatus::Paused.is_active());
        assert!(ExecutionStatus::Sleeping.is_active());
        assert!(!ExecutionStatus::Completed.is_active());
        assert!(!ExecutionStatus::Failed.is_active());
    }

    #[test]
    fn test_execution_context_creation() {
        let fixture = ExecutionContext::new();
        let actual = fixture.attempt;
        let expected = 1;
        assert_eq!(actual, expected);
        assert!(fixture.can_retry());
    }

    #[test]
    fn test_execution_context_with_trigger_event() {
        let fixture_event = json!({"type": "user.created", "data": {"id": 123}});
        let actual = ExecutionContext::new().with_trigger_event(fixture_event.clone());
        assert_eq!(actual.trigger_event, Some(fixture_event));
    }

    #[test]
    fn test_execution_context_can_retry() {
        let mut fixture = ExecutionContext::new();
        assert!(fixture.can_retry());

        fixture.attempt = 3;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_execution_state_creation() {
        let fixture_run_id = ingest_core::generate_id_with_prefix("run");
        let fixture_fn_id = ingest_core::generate_id_with_prefix("fn");
        let actual = ExecutionState::new(fixture_run_id.clone(), fixture_fn_id.clone());

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_fn_id);
        assert_eq!(actual.status, ExecutionStatus::Pending);
        assert_eq!(actual.version, 1);
    }

    #[test]
    fn test_execution_state_update() {
        let mut fixture = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let original_version = fixture.version;
        let original_updated = fixture.updated_at;

        // Add a small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        fixture.update();

        assert_eq!(fixture.version, original_version + 1);
        assert!(fixture.updated_at >= original_updated);
    }

    #[test]
    fn test_execution_state_set_status() {
        let mut fixture = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );

        fixture.set_status(ExecutionStatus::Running);
        assert_eq!(fixture.status, ExecutionStatus::Running);
        assert!(fixture.completed_at.is_none());

        fixture.set_status(ExecutionStatus::Completed);
        assert_eq!(fixture.status, ExecutionStatus::Completed);
        assert!(fixture.completed_at.is_some());
    }

    #[test]
    fn test_execution_state_variables() {
        let mut fixture = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );

        fixture.set_variable("key1", json!("value1"));
        assert_eq!(fixture.get_variable("key1"), Some(&json!("value1")));

        let removed = fixture.remove_variable("key1");
        assert_eq!(removed, Some(json!("value1")));
        assert!(fixture.get_variable("key1").is_none());
    }

    #[test]
    fn test_state_snapshot_creation() {
        let fixture_run_id = ingest_core::generate_id_with_prefix("run");
        let fixture_state = ExecutionState::new(
            fixture_run_id.clone(),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let actual =
            StateSnapshot::new(fixture_run_id.clone(), fixture_state.clone(), "checkpoint1");

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.state, fixture_state);
        assert_eq!(actual.checkpoint_name, "checkpoint1");
        assert!(actual.id.as_str().starts_with("snap_"));
    }

    #[test]
    fn test_state_transition_creation() {
        let fixture_run_id = ingest_core::generate_id_with_prefix("run");
        let actual = StateTransition::new(
            fixture_run_id.clone(),
            ExecutionStatus::Pending,
            ExecutionStatus::Running,
            "Started execution",
            1,
            2,
        );

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.from_status, ExecutionStatus::Pending);
        assert_eq!(actual.to_status, ExecutionStatus::Running);
        assert_eq!(actual.reason, "Started execution");
        assert!(actual.id.as_str().starts_with("trans_"));
    }

    #[test]
    fn test_state_transition_is_valid() {
        let fixture = StateTransition::new(
            ingest_core::generate_id_with_prefix("run"),
            ExecutionStatus::Pending,
            ExecutionStatus::Running,
            "Valid transition",
            1,
            2,
        );
        assert!(fixture.is_valid_transition());

        let invalid_fixture = StateTransition::new(
            ingest_core::generate_id_with_prefix("run"),
            ExecutionStatus::Completed,
            ExecutionStatus::Running,
            "Invalid transition",
            2,
            3,
        );
        assert!(!invalid_fixture.is_valid_transition());
    }

    #[test]
    fn test_execution_state_validation() {
        let fixture = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_state_snapshot_validation() {
        let fixture_state = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let fixture = StateSnapshot::new(
            ingest_core::generate_id_with_prefix("run"),
            fixture_state,
            "valid_checkpoint",
        );
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_state_transition_validation() {
        let fixture = StateTransition::new(
            ingest_core::generate_id_with_prefix("run"),
            ExecutionStatus::Pending,
            ExecutionStatus::Running,
            "Valid transition",
            1,
            2,
        );
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_serialization() {
        let fixture_state = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let actual = serde_json::to_string(&fixture_state);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_deserialization() {
        let fixture_state = ExecutionState::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        let serialized = serde_json::to_string(&fixture_state).unwrap();
        let actual: ExecutionState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.run_id, fixture_state.run_id);
        assert_eq!(actual.status, fixture_state.status);
    }
}
