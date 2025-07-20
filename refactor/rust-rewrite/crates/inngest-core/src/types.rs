use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Identifier for different scopes in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId {
    pub account_id: Uuid,
    pub env_id: Uuid,
    pub app_id: Uuid,
}

/// Unique identifier for a function run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId {
    pub tenant: TenantId,
    pub function_id: Uuid,
    pub run_id: Uuid,
}

/// Generic identifier for state management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StateId(Uuid);

impl StateId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for StateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Priority levels for queue items
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Status of a function run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Paused,
}

/// Status of a step execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Edge in the function execution graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub incoming: String,
    pub outgoing: String,
}

/// Constants used throughout the system
pub mod consts {
    pub const DEV_SERVER_ACCOUNT_ID: &str = "dev-server-account";
    pub const DEV_SERVER_ENV_ID: &str = "dev-server-env";

    pub const DEFAULT_MAX_STEP_LIMIT: usize = 50;
    pub const DEFAULT_MAX_STATE_SIZE_LIMIT: usize = 1024 * 1024; // 1MB

    pub const FN_FINISHED_NAME: &str = "inngest/function.finished";
    pub const FN_FAILED_NAME: &str = "inngest/function.failed";

    pub const INVOKE_CORRELATION_ID: &str = "correlation_id";
}
