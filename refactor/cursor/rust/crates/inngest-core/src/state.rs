//! State management types and traits for Inngest

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Identifier, Metadata, Result};

/// State interface for workflow runs
/// This corresponds to the Go `state.State` interface
pub trait State: Send + Sync {
    /// Get the run metadata
    fn metadata(&self) -> &Metadata;

    /// Get the identifier for this function run
    fn identifier(&self) -> &Identifier;

    /// Get the stack of completed step IDs
    fn stack(&self) -> &[String];

    /// Get the root event that triggered the workflow
    fn event(&self) -> &serde_json::Value;

    /// Get all events associated with this workflow run
    fn events(&self) -> &[serde_json::Value];

    /// Get all completed step outputs
    fn actions(&self) -> &HashMap<String, serde_json::Value>;

    /// Get all step errors
    fn errors(&self) -> &HashMap<String, String>;

    /// Get the output or error for a specific step ID
    fn action_by_id(&self, id: &str) -> Result<Option<&serde_json::Value>>;

    /// Check if a step has completed successfully
    fn action_complete(&self, id: &str) -> bool;

    /// Get the cron schedule if this is a cron-triggered run
    fn cron_schedule(&self) -> Option<&str>;

    /// Check if this is a cron-triggered run
    fn is_cron(&self) -> bool;
}

/// Memoized step output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoizedStep {
    /// Step ID
    pub id: String,

    /// Step output data
    pub data: serde_json::Value,

    /// When this step completed
    pub completed_at: DateTime<Utc>,

    /// Step name (optional)
    pub name: Option<String>,
}

impl MemoizedStep {
    /// Create a new memoized step
    pub fn new(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            data,
            completed_at: Utc::now(),
            name: None,
        }
    }

    /// Set the step name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

/// In-memory state implementation
#[derive(Debug, Clone)]
pub struct MemoryState {
    metadata: Metadata,
    events: Vec<serde_json::Value>,
    actions: HashMap<String, serde_json::Value>,
    errors: HashMap<String, String>,
    stack: Vec<String>,
    cron_schedule: Option<String>,
}

impl MemoryState {
    /// Create a new memory state
    pub fn new(
        metadata: Metadata,
        events: Vec<serde_json::Value>,
        actions: Vec<MemoizedStep>,
        stack: Vec<String>,
    ) -> Self {
        let actions_map = actions
            .into_iter()
            .map(|step| (step.id, step.data))
            .collect();

        Self {
            metadata,
            events,
            actions: actions_map,
            errors: HashMap::new(),
            stack,
            cron_schedule: None,
        }
    }

    /// Set the cron schedule
    pub fn with_cron_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.cron_schedule = Some(schedule.into());
        self
    }

    /// Add a step error
    pub fn add_error(&mut self, step_id: impl Into<String>, error: impl Into<String>) {
        self.errors.insert(step_id.into(), error.into());
    }

    /// Add a completed step
    pub fn add_action(&mut self, step_id: impl Into<String>, data: serde_json::Value) {
        let id = step_id.into();
        self.actions.insert(id.clone(), data);
        if !self.stack.contains(&id) {
            self.stack.push(id);
        }
    }
}

impl State for MemoryState {
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    fn identifier(&self) -> &Identifier {
        &self.metadata.identifier
    }

    fn stack(&self) -> &[String] {
        &self.stack
    }

    fn event(&self) -> &serde_json::Value {
        self.events.first().unwrap_or(&serde_json::Value::Null)
    }

    fn events(&self) -> &[serde_json::Value] {
        &self.events
    }

    fn actions(&self) -> &HashMap<String, serde_json::Value> {
        &self.actions
    }

    fn errors(&self) -> &HashMap<String, String> {
        &self.errors
    }

    fn action_by_id(&self, id: &str) -> Result<Option<&serde_json::Value>> {
        Ok(self.actions.get(id))
    }

    fn action_complete(&self, id: &str) -> bool {
        self.actions.contains_key(id) && !self.errors.contains_key(id)
    }

    fn cron_schedule(&self) -> Option<&str> {
        self.cron_schedule.as_deref()
    }

    fn is_cron(&self) -> bool {
        self.cron_schedule.is_some()
    }
}

/// State loader interface
#[async_trait]
pub trait StateLoader: Send + Sync {
    /// Load state for a given identifier
    async fn load(&self, id: &Identifier) -> Result<Box<dyn State>>;

    /// Check if state exists for a given identifier
    async fn exists(&self, id: &Identifier) -> Result<bool>;
}

/// State mutator interface
#[async_trait]
pub trait StateMutator: Send + Sync {
    /// Create new state for a run
    async fn create(&self, input: CreateStateInput) -> Result<Box<dyn State>>;

    /// Save a step's output to state
    async fn save_step(&self, id: &Identifier, step_id: &str, data: &[u8]) -> Result<bool>;

    /// Delete state for a given identifier
    async fn delete(&self, id: &Identifier) -> Result<bool>;
}

/// Combined state manager interface
#[async_trait]
pub trait StateManager: StateLoader + StateMutator + Send + Sync + Clone {
    /// Get the name of this state manager implementation
    fn name(&self) -> &str;
}

/// Input for creating new state
#[derive(Debug, Clone)]
pub struct CreateStateInput {
    /// Run identifier
    pub identifier: Identifier,

    /// Initial metadata
    pub metadata: Metadata,

    /// Initial events
    pub events: Vec<serde_json::Value>,

    /// Optional initial step data
    pub initial_steps: Vec<MemoizedStep>,
}

impl CreateStateInput {
    /// Create new state input
    pub fn new(identifier: Identifier, metadata: Metadata, events: Vec<serde_json::Value>) -> Self {
        Self {
            identifier,
            metadata,
            events,
            initial_steps: Vec::new(),
        }
    }

    /// Add initial step data
    pub fn with_initial_steps(mut self, steps: Vec<MemoizedStep>) -> Self {
        self.initial_steps = steps;
        self
    }
}

/// Pause information for waiting functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pause {
    /// Pause ID
    pub id: String,

    /// Run identifier this pause belongs to
    pub identifier: Identifier,

    /// Event name to wait for
    pub event_name: String,

    /// Optional condition expression
    pub expression: Option<String>,

    /// Timeout for the pause
    pub timeout: DateTime<Utc>,

    /// When this pause was created
    pub created_at: DateTime<Utc>,
}

impl Pause {
    /// Create a new pause
    pub fn new(
        identifier: Identifier,
        event_name: impl Into<String>,
        timeout: DateTime<Utc>,
    ) -> Self {
        Self {
            id: format!("{}-{}", identifier.run_id, uuid::Uuid::new_v4()),
            identifier,
            event_name: event_name.into(),
            expression: None,
            timeout,
            created_at: Utc::now(),
        }
    }

    /// Set the condition expression
    pub fn with_expression(mut self, expression: impl Into<String>) -> Self {
        self.expression = Some(expression.into());
        self
    }

    /// Check if this pause has expired
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now > self.timeout
    }
}

/// Pause manager interface
#[async_trait]
pub trait PauseManager: Send + Sync {
    /// Save a pause
    async fn save_pause(&self, pause: &Pause) -> Result<()>;

    /// Consume a pause (remove it and return the identifier)
    async fn consume_pause(&self, pause_id: &str) -> Result<Option<Identifier>>;

    /// Find pauses matching an event
    async fn find_pauses(&self, event_name: &str) -> Result<Vec<Pause>>;

    /// Check if a pause exists
    async fn pause_exists(&self, pause_id: &str) -> Result<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, Metadata, metadata::RunStatus};
    use ulid::Ulid;
    use uuid::Uuid;

    fn create_test_identifier() -> Identifier {
        Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            1,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
    }

    fn create_test_metadata() -> Metadata {
        Metadata::new(create_test_identifier(), RunStatus::Running, Utc::now())
    }

    #[test]
    fn test_memoized_step() {
        let step = MemoizedStep::new("step1", serde_json::json!({"result": "success"}))
            .with_name("Test Step");

        assert_eq!(step.id, "step1");
        assert_eq!(step.name, Some("Test Step".to_string()));
        assert_eq!(step.data, serde_json::json!({"result": "success"}));
    }

    #[test]
    fn test_memory_state() {
        let metadata = create_test_metadata();
        let events = vec![serde_json::json!({"name": "test.event"})];
        let actions = vec![MemoizedStep::new(
            "step1",
            serde_json::json!({"result": "success"}),
        )];
        let stack = vec!["step1".to_string()];

        let state = MemoryState::new(metadata.clone(), events.clone(), actions, stack);

        assert_eq!(state.metadata(), &metadata);
        assert_eq!(state.events(), &events);
        assert!(state.action_complete("step1"));
        assert!(!state.action_complete("step2"));
        assert_eq!(state.stack().len(), 1);
    }

    #[test]
    fn test_pause_creation() {
        let identifier = create_test_identifier();
        let timeout = Utc::now() + chrono::Duration::minutes(5);

        let pause = Pause::new(identifier.clone(), "user.created", timeout)
            .with_expression("event.data.user_id == 'test'");

        assert_eq!(pause.identifier, identifier);
        assert_eq!(pause.event_name, "user.created");
        assert_eq!(
            pause.expression,
            Some("event.data.user_id == 'test'".to_string())
        );
        assert!(!pause.is_expired(Utc::now()));
    }

    #[test]
    fn test_create_state_input() {
        let identifier = create_test_identifier();
        let metadata = create_test_metadata();
        let events = vec![serde_json::json!({"name": "test.event"})];

        let input = CreateStateInput::new(identifier.clone(), metadata.clone(), events.clone());

        assert_eq!(input.identifier, identifier);
        assert_eq!(input.metadata, metadata);
        assert_eq!(input.events, events);
        assert!(input.initial_steps.is_empty());
    }
}
