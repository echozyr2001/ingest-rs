//! State management for Inngest
//!
//! This module provides state management capabilities for function execution,
//! including state persistence, metadata management, and run lifecycle tracking.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use inngest_core::{Result, StateIdentifier, Ulid, Uuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Run status enumeration matching Go's enums.RunStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(rename = "PAUSED")]
    Paused,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Running => write!(f, "RUNNING"),
            RunStatus::Completed => write!(f, "COMPLETED"),
            RunStatus::Failed => write!(f, "FAILED"),
            RunStatus::Cancelled => write!(f, "CANCELLED"),
            RunStatus::Paused => write!(f, "PAUSED"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = inngest_core::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "RUNNING" => Ok(RunStatus::Running),
            "COMPLETED" => Ok(RunStatus::Completed),
            "FAILED" => Ok(RunStatus::Failed),
            "CANCELLED" => Ok(RunStatus::Cancelled),
            "PAUSED" => Ok(RunStatus::Paused),
            _ => Err(inngest_core::Error::InvalidData(format!(
                "Invalid run status: {}",
                s
            ))),
        }
    }
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus::Running
    }
}

/// State identifier for function runs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StateId {
    /// Unique run ID
    pub run_id: Ulid,
    /// Function ID
    pub function_id: Uuid,
    /// Tenant information
    pub tenant: Tenant,
}

/// Tenant information for multi-tenancy support
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Tenant {
    /// Account ID
    pub account_id: Uuid,
    /// Environment/Workspace ID  
    pub env_id: Uuid,
    /// Application ID
    pub app_id: Uuid,
}

/// Function run metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// State identifier
    pub id: StateId,
    /// Current run status
    pub status: RunStatus,
    /// Function version when the run started
    pub function_version: i32,
    /// Event ID that triggered this run
    pub event_id: Ulid,
    /// All event IDs associated with this run
    pub event_ids: Vec<Ulid>,
    /// Batch ID if this is a batch function
    pub batch_id: Option<Ulid>,
    /// Original run ID if this is a rerun
    pub original_run_id: Option<Ulid>,
    /// When the function started
    pub started_at: DateTime<Utc>,
    /// Whether this was started via debugger
    pub debugger: bool,
    /// Run type (e.g., "retry", "manual")
    pub run_type: Option<String>,
    /// Request version for SDK compatibility
    pub request_version: i32,
    /// Additional context data
    pub context: HashMap<String, serde_json::Value>,
    /// Tracing span ID
    pub span_id: Option<String>,
    /// Custom idempotency key
    pub key: Option<String>,
    /// Priority factor for scheduling
    pub priority_factor: Option<i64>,
}

impl Metadata {
    pub fn new(id: StateId, event_id: Ulid) -> Self {
        Self {
            id,
            status: RunStatus::Running,
            function_version: 1,
            event_id,
            event_ids: vec![event_id],
            batch_id: None,
            original_run_id: None,
            started_at: Utc::now(),
            debugger: false,
            run_type: None,
            request_version: 1,
            context: HashMap::new(),
            span_id: None,
            key: None,
            priority_factor: None,
        }
    }

    /// Generate idempotency key for this run
    pub fn idempotency_key(&self) -> String {
        if let Some(key) = &self.key {
            key.clone()
        } else {
            format!("{}:{}", self.id.function_id, self.id.run_id)
        }
    }
}

/// Memoized step data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoizedStep {
    /// Step ID
    pub id: String,
    /// Step output data
    pub data: serde_json::Value,
}

/// State creation input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStateInput {
    /// State identifier
    pub id: StateId,
    /// Events that triggered this run (JSON-encoded)
    pub events: Vec<serde_json::Value>,
    /// Pre-memoized steps
    pub steps: Vec<MemoizedStep>,
    /// Step inputs for resume scenarios
    pub step_inputs: Vec<MemoizedStep>,
    /// Initial metadata
    pub metadata: Metadata,
}

/// Complete state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// State metadata
    pub metadata: Metadata,
    /// Trigger events (JSON-encoded)
    pub events: Vec<serde_json::Value>,
    /// Step execution stack (ordered step IDs)
    pub stack: Vec<String>,
    /// Step outputs keyed by step ID
    pub steps: HashMap<String, serde_json::Value>,
    /// Step inputs keyed by step ID
    pub step_inputs: HashMap<String, serde_json::Value>,
}

impl State {
    pub fn new(input: CreateStateInput) -> Self {
        let mut steps = HashMap::new();
        let mut step_inputs = HashMap::new();
        let mut stack = Vec::new();

        // Load pre-memoized steps
        for step in input.steps {
            steps.insert(step.id.clone(), step.data);
            stack.push(step.id);
        }

        // Load step inputs
        for step_input in input.step_inputs {
            step_inputs.insert(step_input.id, step_input.data);
        }

        Self {
            metadata: input.metadata,
            events: input.events,
            stack,
            steps,
            step_inputs,
        }
    }

    /// Get the primary event that triggered this run
    pub fn event(&self) -> Option<&serde_json::Value> {
        self.events.first()
    }

    /// Check if a step has completed
    pub fn step_completed(&self, step_id: &str) -> bool {
        self.steps.contains_key(step_id)
    }

    /// Get step output data
    pub fn step_data(&self, step_id: &str) -> Option<&serde_json::Value> {
        self.steps.get(step_id)
    }

    /// Get all step outputs as a map
    pub fn actions(&self) -> &HashMap<String, serde_json::Value> {
        &self.steps
    }

    /// Add a step to the execution stack
    pub fn add_step(&mut self, step_id: String, data: serde_json::Value) {
        self.steps.insert(step_id.clone(), data);
        if !self.stack.contains(&step_id) {
            self.stack.push(step_id);
        }
    }
}

/// Metadata update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataUpdate {
    /// Request version from SDK
    pub request_version: Option<i32>,
    /// Started at timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Whether immediate execution is disabled
    pub disable_immediate_execution: Option<bool>,
    /// Whether the function uses AI
    pub has_ai: Option<bool>,
}

/// State management trait for loading and storing function execution state
#[async_trait]
pub trait StateManager: Send + Sync {
    /// Create new state for a function run
    async fn create(&self, input: CreateStateInput) -> Result<State>;

    /// Load existing state by ID
    async fn load(&self, id: &StateId) -> Result<Option<State>>;

    /// Load only metadata by ID
    async fn load_metadata(&self, id: &StateId) -> Result<Option<Metadata>>;

    /// Update metadata for an existing run
    async fn update_metadata(&self, id: &StateId, update: MetadataUpdate) -> Result<()>;

    /// Save step output to state
    async fn save_step(&self, id: &StateId, step_id: &str, data: serde_json::Value)
        -> Result<bool>;

    /// Set run status
    async fn set_status(&self, id: &StateId, status: RunStatus) -> Result<()>;

    /// Delete state (for cleanup)
    async fn delete(&self, id: &StateId) -> Result<bool>;

    /// Check if state exists
    async fn exists(&self, id: &StateId) -> Result<bool>;

    // Legacy StateManager trait compatibility
    async fn save_execution_state(
        &self,
        _state_id: &StateIdentifier,
        data: &serde_json::Value,
    ) -> Result<()> {
        let id = StateId {
            run_id: Ulid::new(), // This is a simplified mapping
            function_id: Uuid::new_v4(),
            tenant: Tenant {
                account_id: Uuid::new_v4(),
                env_id: Uuid::new_v4(),
                app_id: Uuid::new_v4(),
            },
        };
        self.save_step(&id, &_state_id.run_id, data.clone()).await?;
        Ok(())
    }

    async fn load_execution_state(
        &self,
        _state_id: &StateIdentifier,
    ) -> Result<Option<serde_json::Value>> {
        // Simplified implementation for compatibility
        Ok(None)
    }

    async fn delete_execution_state(&self, _state_id: &StateIdentifier) -> Result<()> {
        // Simplified implementation for compatibility
        Ok(())
    }
}

/// In-memory state manager for testing and development
pub struct InMemoryStateManager {
    states: std::sync::Arc<tokio::sync::RwLock<HashMap<StateId, State>>>,
}

impl InMemoryStateManager {
    pub fn new() -> Self {
        Self {
            states: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateManager for InMemoryStateManager {
    async fn create(&self, input: CreateStateInput) -> Result<State> {
        let state = State::new(input);
        let mut states = self.states.write().await;

        // Check for existing state (idempotency)
        if states.contains_key(&state.metadata.id) {
            return Ok(states.get(&state.metadata.id).unwrap().clone());
        }

        states.insert(state.metadata.id.clone(), state.clone());
        Ok(state)
    }

    async fn load(&self, id: &StateId) -> Result<Option<State>> {
        let states = self.states.read().await;
        Ok(states.get(id).cloned())
    }

    async fn load_metadata(&self, id: &StateId) -> Result<Option<Metadata>> {
        let states = self.states.read().await;
        Ok(states.get(id).map(|s| s.metadata.clone()))
    }

    async fn update_metadata(&self, id: &StateId, update: MetadataUpdate) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(id) {
            if let Some(rv) = update.request_version {
                state.metadata.request_version = rv;
            }
            if let Some(started_at) = update.started_at {
                state.metadata.started_at = started_at;
            }
            // Handle other update fields...
        }
        Ok(())
    }

    async fn save_step(
        &self,
        id: &StateId,
        step_id: &str,
        data: serde_json::Value,
    ) -> Result<bool> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(id) {
            state.add_step(step_id.to_string(), data);
            return Ok(true);
        }
        Ok(false)
    }

    async fn set_status(&self, id: &StateId, status: RunStatus) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(id) {
            state.metadata.status = status;
        }
        Ok(())
    }

    async fn delete(&self, id: &StateId) -> Result<bool> {
        let mut states = self.states.write().await;
        Ok(states.remove(id).is_some())
    }

    async fn exists(&self, id: &StateId) -> Result<bool> {
        let states = self.states.read().await;
        Ok(states.contains_key(id))
    }
}

/// Redis-based state manager (placeholder for full implementation)
#[allow(dead_code)]
pub struct RedisStateManager {
    client: redis::Client,
}

impl RedisStateManager {
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).map_err(|e| {
            inngest_core::Error::Redis(format!("Failed to create Redis client: {}", e))
        })?;

        Ok(Self { client })
    }

    #[allow(dead_code)]
    fn state_key(&self, id: &StateId) -> String {
        format!(
            "inngest:state:{}:{}:{}",
            id.tenant.account_id, id.function_id, id.run_id
        )
    }

    #[allow(dead_code)]
    fn metadata_key(&self, id: &StateId) -> String {
        format!(
            "inngest:meta:{}:{}:{}",
            id.tenant.account_id, id.function_id, id.run_id
        )
    }

    #[allow(dead_code)]
    fn events_key(&self, id: &StateId) -> String {
        format!(
            "inngest:events:{}:{}:{}",
            id.tenant.account_id, id.function_id, id.run_id
        )
    }

    #[allow(dead_code)]
    fn steps_key(&self, id: &StateId) -> String {
        format!(
            "inngest:steps:{}:{}:{}",
            id.tenant.account_id, id.function_id, id.run_id
        )
    }

    #[allow(dead_code)]
    fn stack_key(&self, id: &StateId) -> String {
        format!(
            "inngest:stack:{}:{}:{}",
            id.tenant.account_id, id.function_id, id.run_id
        )
    }

    #[allow(dead_code)]
    fn idempotency_key(&self, idempotency: &str, tenant: &Tenant) -> String {
        format!("inngest:idem:{}:{}", tenant.account_id, idempotency)
    }
}

#[async_trait]
impl StateManager for RedisStateManager {
    async fn create(&self, input: CreateStateInput) -> Result<State> {
        // TODO: Implement Redis-based state creation with Lua scripts for atomicity
        // This is a placeholder implementation
        let state = State::new(input);
        Ok(state)
    }

    async fn load(&self, _id: &StateId) -> Result<Option<State>> {
        // TODO: Implement Redis-based state loading
        Ok(None)
    }

    async fn load_metadata(&self, _id: &StateId) -> Result<Option<Metadata>> {
        // TODO: Implement Redis-based metadata loading
        Ok(None)
    }

    async fn update_metadata(&self, _id: &StateId, _update: MetadataUpdate) -> Result<()> {
        // TODO: Implement Redis-based metadata updates
        Ok(())
    }

    async fn save_step(
        &self,
        _id: &StateId,
        _step_id: &str,
        _data: serde_json::Value,
    ) -> Result<bool> {
        // TODO: Implement Redis-based step saving with Lua scripts
        Ok(true)
    }

    async fn set_status(&self, _id: &StateId, _status: RunStatus) -> Result<()> {
        // TODO: Implement Redis-based status updates
        Ok(())
    }

    async fn delete(&self, _id: &StateId) -> Result<bool> {
        // TODO: Implement Redis-based state deletion
        Ok(true)
    }

    async fn exists(&self, _id: &StateId) -> Result<bool> {
        // TODO: Implement Redis-based existence check
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_in_memory_state_manager() {
        let fixture = InMemoryStateManager::new();
        let state_id = StateId {
            run_id: Ulid::new(),
            function_id: Uuid::new_v4(),
            tenant: Tenant {
                account_id: Uuid::new_v4(),
                env_id: Uuid::new_v4(),
                app_id: Uuid::new_v4(),
            },
        };

        // Test state creation
        let input = CreateStateInput {
            id: state_id.clone(),
            events: vec![serde_json::json!({"name": "test.event", "data": {}})],
            steps: vec![],
            step_inputs: vec![],
            metadata: Metadata::new(state_id.clone(), Ulid::new()),
        };

        let actual = fixture.create(input).await.unwrap();
        let expected = true;
        assert_eq!(actual.metadata.id, state_id);

        // Test state loading
        let loaded_state = fixture.load(&state_id).await.unwrap();
        assert!(loaded_state.is_some());

        // Test step saving
        let step_saved = fixture
            .save_step(&state_id, "step1", serde_json::json!({"output": "value"}))
            .await
            .unwrap();
        assert_eq!(step_saved, expected);

        // Verify step was saved
        let updated_state = fixture.load(&state_id).await.unwrap().unwrap();
        assert!(updated_state.step_completed("step1"));
    }

    #[test]
    fn test_metadata_idempotency_key() {
        let fixture = Metadata {
            id: StateId {
                run_id: Ulid::new(),
                function_id: Uuid::new_v4(),
                tenant: Tenant {
                    account_id: Uuid::new_v4(),
                    env_id: Uuid::new_v4(),
                    app_id: Uuid::new_v4(),
                },
            },
            key: Some("custom-key".to_string()),
            status: RunStatus::Running,
            function_version: 1,
            event_id: Ulid::new(),
            event_ids: vec![],
            batch_id: None,
            original_run_id: None,
            started_at: Utc::now(),
            debugger: false,
            run_type: None,
            request_version: 1,
            context: HashMap::new(),
            span_id: None,
            priority_factor: None,
        };

        let actual = fixture.idempotency_key();
        let expected = "custom-key";
        assert_eq!(actual, expected);
    }
}
