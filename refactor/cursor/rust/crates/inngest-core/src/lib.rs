//! Core types and traits for Inngest
//!
//! This crate provides the fundamental types, traits, and error handling
//! that are used throughout the Inngest system.

pub mod error;
pub mod event;
pub mod traits;

// Re-export commonly used types
pub use error::{Error, Result};
pub use event::{Event, InngestMetadata, TrackedEvent};
pub use traits::{EventHandler, QueueManager, StateManager};

// Re-export external types that are used frequently
pub use chrono::{DateTime, Utc};
pub use ulid::Ulid;
pub use uuid::Uuid;

// Function-related types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub id: Uuid,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub steps: Vec<Step>,
    pub concurrency: Option<ConcurrencyConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Trigger {
    pub event: String,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConcurrencyConfig {
    pub limit: u32,
    pub scope: String,
}

// Basic types that other crates might need
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateIdentifier {
    pub workflow_id: String,
    pub run_id: String,
}

impl StateIdentifier {
    pub fn new(workflow_id: String, run_id: String) -> Self {
        Self {
            workflow_id,
            run_id,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionContext {
    pub function_id: String,
    pub run_id: String,
    pub step_id: String,
    pub attempt: u32,
    pub state: StateIdentifier,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub status: ExecutionStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub retry_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Waiting,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types() {
        let event = Event {
            name: "test.event".to_string(),
            data: serde_json::json!({"key": "value"}),
            user: None,
            id: Some("test-id".to_string()),
            ulid: Ulid::new(),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
            version: Some("2023-05-15".to_string()),
        };

        assert_eq!(event.name, "test.event");
        assert!(event.data.is_object());
    }
}
