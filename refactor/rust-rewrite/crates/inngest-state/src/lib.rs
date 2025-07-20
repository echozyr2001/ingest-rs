use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use inngest_core::{RunStatus, StateId, StepStatus};

pub mod memory_state;
pub mod postgres_state;

pub use memory_state::*;
pub use postgres_state::*;

// TODO: Implement when we add database support
// pub mod postgres_state;
// pub mod redis_state;

/// Metadata for a function run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    /// Unique identifier for this run
    pub id: StateId,
    /// Function ID being executed
    pub function_id: Uuid,
    /// Function version
    pub function_version: u32,
    /// Current run status
    pub status: RunStatus,
    /// When the run started
    pub started_at: Option<DateTime<Utc>>,
    /// When the run ended
    pub ended_at: Option<DateTime<Utc>>,
    /// Event IDs that triggered this run
    pub event_ids: Vec<Uuid>,
    /// Idempotency key
    pub idempotency_key: Option<String>,
    /// Batch ID if this is part of a batch
    pub batch_id: Option<Uuid>,
}

/// Step execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    /// Step identifier
    pub id: String,
    /// Step status
    pub status: StepStatus,
    /// Step input data
    pub input: Option<serde_json::Value>,
    /// Step output data
    pub output: Option<serde_json::Value>,
    /// Error information if step failed
    pub error: Option<String>,
    /// Step attempt number
    pub attempt: u32,
    /// When step started
    pub started_at: Option<DateTime<Utc>>,
    /// When step completed
    pub completed_at: Option<DateTime<Utc>>,
}

/// Complete run state including all steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    /// Run metadata
    pub metadata: RunMetadata,
    /// All step states
    pub steps: Vec<StepState>,
    /// Function execution stack
    pub stack: Vec<serde_json::Value>,
    /// Current execution index
    pub ctx: Option<serde_json::Value>,
}

/// State management errors
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Run not found: {id}")]
    RunNotFound { id: StateId },

    #[error("Step not found: {step_id} in run {run_id}")]
    StepNotFound { step_id: String, run_id: StateId },

    #[error("Identifier already exists: {id}")]
    IdentifierExists { id: StateId },

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: RunStatus, to: RunStatus },

    #[error("Concurrency conflict")]
    ConcurrencyConflict,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// State manager trait for persisting function execution state
#[async_trait]
pub trait StateManager: Send + Sync {
    /// Create a new function run
    async fn create_run(&self, metadata: &RunMetadata) -> Result<(), StateError>;

    /// Load complete run state
    async fn load_run(&self, id: &StateId) -> Result<RunState, StateError>;

    /// Update run metadata
    async fn update_run_metadata(&self, metadata: &RunMetadata) -> Result<(), StateError>;

    /// Update run status
    async fn update_run_status(&self, id: &StateId, status: RunStatus) -> Result<(), StateError>;

    /// Add or update a step state
    async fn save_step(&self, run_id: &StateId, step: &StepState) -> Result<(), StateError>;

    /// Get step state
    async fn load_step(&self, run_id: &StateId, step_id: &str) -> Result<StepState, StateError>;

    /// Load events for a run
    async fn load_events(&self, id: &StateId) -> Result<Vec<serde_json::Value>, StateError>;

    /// Save events for a run
    async fn save_events(
        &self,
        id: &StateId,
        events: &[serde_json::Value],
    ) -> Result<(), StateError>;

    /// Delete run state (for cleanup)
    async fn delete_run(&self, id: &StateId) -> Result<(), StateError>;

    /// Check if run exists
    async fn run_exists(&self, id: &StateId) -> Result<bool, StateError>;

    /// List runs by function ID
    async fn list_runs_by_function(
        &self,
        function_id: Uuid,
    ) -> Result<Vec<RunMetadata>, StateError>;

    /// List active runs
    async fn list_active_runs(&self) -> Result<Vec<RunMetadata>, StateError>;
}

/// Mutable configuration for run metadata updates
#[derive(Debug, Clone)]
pub struct MutableConfig {
    pub started_at: Option<DateTime<Utc>>,
    pub force_step_plan: Option<bool>,
    pub request_version: Option<String>,
}

/// Extended state manager with additional operations
#[async_trait]
pub trait StateManagerV2: StateManager {
    /// Update mutable metadata fields
    async fn update_metadata(&self, id: &StateId, config: MutableConfig) -> Result<(), StateError>;

    /// Atomic compare-and-swap operation for state
    async fn cas_run_status(
        &self,
        id: &StateId,
        expected: RunStatus,
        new: RunStatus,
    ) -> Result<bool, StateError>;

    /// Get run statistics
    async fn get_run_stats(&self, id: &StateId) -> Result<RunStats, StateError>;
}

/// Statistics about a function run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStats {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub pending_steps: usize,
    pub total_duration: Option<chrono::Duration>,
    pub state_size_bytes: usize,
}
