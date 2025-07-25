//! Metadata for Inngest workflow runs

use crate::Identifier;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Run status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Function is currently running
    Running,
    /// Function completed successfully
    Completed,
    /// Function failed with an error
    Failed,
    /// Function was cancelled
    Cancelled,
    /// Function is paused (sleeping or waiting for event)
    Paused,
}

impl RunStatus {
    /// Check if this status represents a completed run (finished, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }

    /// Check if this status represents an active run
    pub fn is_active(&self) -> bool {
        matches!(self, RunStatus::Running | RunStatus::Paused)
    }
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

/// Metadata for a workflow run
/// This corresponds to the Go `state.Metadata` struct
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    /// Identifier stores the full identifier for the run
    #[serde(rename = "id")]
    pub identifier: Identifier,

    /// Status returns the function status for this run
    pub status: RunStatus,

    /// Debugger represents whether this function was started via the debugger
    #[serde(default)]
    pub debugger: bool,

    /// RunType indicates the run type for this particular flow
    #[serde(rename = "runType", skip_serializing_if = "Option::is_none")]
    pub run_type: Option<String>,

    /// Name stores the name of the workflow as it started (deprecated)
    #[serde(default)]
    pub name: String,

    /// Version represents the version of metadata in particular
    #[serde(default)]
    pub version: i32,

    /// StartedAt records the time when the function started
    #[serde(rename = "sat")]
    pub started_at: DateTime<Utc>,

    /// RequestVersion represents the executor request versioning/hashing style
    /// used to manage state.
    ///
    /// TS v3, Go, Rust, Elixir, and Java all use the same hashing style (1).
    /// TS v1 + v2 use a unique hashing style (0).
    #[serde(rename = "requestVersion", default)]
    pub request_version: i32,

    /// Context stores additional context data
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub context: std::collections::HashMap<String, serde_json::Value>,

    /// SpanID for distributed tracing
    #[serde(rename = "spanID", default, skip_serializing_if = "String::is_empty")]
    pub span_id: String,
}

impl Metadata {
    /// Create new metadata for a run
    pub fn new(identifier: Identifier, status: RunStatus, started_at: DateTime<Utc>) -> Self {
        Self {
            identifier,
            status,
            debugger: false,
            run_type: None,
            name: String::new(),
            version: 1,
            started_at,
            request_version: 1, // Default to version 1 for Rust
            context: std::collections::HashMap::new(),
            span_id: String::new(),
        }
    }

    /// Create metadata for a debug run
    pub fn debug(identifier: Identifier, status: RunStatus, started_at: DateTime<Utc>) -> Self {
        let mut metadata = Self::new(identifier, status, started_at);
        metadata.debugger = true;
        metadata
    }

    /// Set the run type
    pub fn with_run_type(mut self, run_type: impl Into<String>) -> Self {
        self.run_type = Some(run_type.into());
        self
    }

    /// Set the name (deprecated)
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the span ID for tracing
    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = span_id.into();
        self
    }

    /// Add context data
    pub fn with_context(
        mut self,
        context: std::collections::HashMap<String, serde_json::Value>,
    ) -> Self {
        self.context = context;
        self
    }

    /// Update the status
    pub fn update_status(&mut self, status: RunStatus) {
        self.status = status;
    }

    /// Check if the run has finished
    pub fn is_finished(&self) -> bool {
        self.status.is_finished()
    }

    /// Check if the run is active
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Get the duration since start (if run is finished)
    pub fn duration(&self) -> Option<chrono::Duration> {
        if self.is_finished() {
            Some(Utc::now() - self.started_at)
        } else {
            None
        }
    }

    /// Get the elapsed time since start
    pub fn elapsed(&self) -> chrono::Duration {
        Utc::now() - self.started_at
    }
}

impl std::fmt::Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Metadata[{}: {} ({})]",
            self.identifier.run_id,
            self.status,
            self.started_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Identifier;
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

    #[test]
    fn test_metadata_creation() {
        let identifier = create_test_identifier();
        let started_at = Utc::now();

        let metadata = Metadata::new(identifier.clone(), RunStatus::Running, started_at);

        assert_eq!(metadata.identifier, identifier);
        assert_eq!(metadata.status, RunStatus::Running);
        assert_eq!(metadata.started_at, started_at);
        assert!(!metadata.debugger);
        assert_eq!(metadata.request_version, 1);
    }

    #[test]
    fn test_debug_metadata() {
        let identifier = create_test_identifier();
        let started_at = Utc::now();

        let metadata = Metadata::debug(identifier, RunStatus::Running, started_at);

        assert!(metadata.debugger);
    }

    #[test]
    fn test_status_checks() {
        assert!(RunStatus::Completed.is_finished());
        assert!(RunStatus::Failed.is_finished());
        assert!(RunStatus::Cancelled.is_finished());
        assert!(!RunStatus::Running.is_finished());
        assert!(!RunStatus::Paused.is_finished());

        assert!(RunStatus::Running.is_active());
        assert!(RunStatus::Paused.is_active());
        assert!(!RunStatus::Completed.is_active());
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = Metadata::new(create_test_identifier(), RunStatus::Running, Utc::now());

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_metadata_builder_pattern() {
        let metadata = Metadata::new(create_test_identifier(), RunStatus::Running, Utc::now())
            .with_run_type("manual")
            .with_name("test-function")
            .with_span_id("span-123");

        assert_eq!(metadata.run_type, Some("manual".to_string()));
        assert_eq!(metadata.name, "test-function");
        assert_eq!(metadata.span_id, "span-123");
    }
}
