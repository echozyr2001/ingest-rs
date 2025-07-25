//! Unique identifiers for Inngest workflow runs

use serde::{Deserialize, Serialize};
use ulid::Ulid;
use uuid::Uuid;
use xxhash_rust::xxh64::xxh64;

/// Identifier represents the unique identifier for a workflow run.
/// This corresponds to the Go `state.Identifier` struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identifier {
    /// RunID is the unique identifier for this specific run instance
    #[serde(rename = "runID")]
    pub run_id: Ulid,

    /// WorkflowID tracks the internal ID of the function
    #[serde(rename = "wID")]
    pub workflow_id: Uuid,

    /// WorkflowVersion tracks the version of the function that was live
    /// at the time of the trigger
    #[serde(rename = "wv")]
    pub workflow_version: i32,

    /// EventID tracks the event ID that started the function
    #[serde(rename = "evtID")]
    pub event_id: Ulid,

    /// BatchID tracks the batch ID for the function, if the function uses batching
    #[serde(rename = "bID", skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<Ulid>,

    /// EventIDs tracks all the events associated with the function run
    #[serde(rename = "eventIDs")]
    pub event_ids: Vec<Ulid>,

    /// Key represents a unique user-defined key to be used as part of the
    /// idempotency key. This is appended to the workflow ID and workflow
    /// version to create a full idempotency key (via the idempotency_key() method).
    ///
    /// If this is not present the RunID is used as this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// AccountID represents the account ID for this run
    #[serde(rename = "aID")]
    pub account_id: Uuid,

    /// WorkspaceID represents the workspace ID for this run
    #[serde(rename = "wsID")]
    pub workspace_id: Uuid,

    /// AppID represents the app ID for this run
    #[serde(rename = "appID")]
    pub app_id: Uuid,

    /// If this is a rerun, the original run ID is stored here
    #[serde(rename = "oRunID", skip_serializing_if = "Option::is_none")]
    pub original_run_id: Option<Ulid>,

    /// ReplayID stores the ID of the replay, if this identifier belongs to a replay
    #[serde(rename = "replayID", skip_serializing_if = "Option::is_none")]
    pub replay_id: Option<Uuid>,

    /// PriorityFactor is used to adjust the priority of this run
    #[serde(rename = "pf", skip_serializing_if = "Option::is_none")]
    pub priority_factor: Option<i64>,
}

impl Identifier {
    /// Create a new identifier with the minimum required fields
    pub fn new(
        run_id: Ulid,
        workflow_id: Uuid,
        workflow_version: i32,
        event_id: Ulid,
        account_id: Uuid,
        workspace_id: Uuid,
        app_id: Uuid,
    ) -> Self {
        Self {
            run_id,
            workflow_id,
            workflow_version,
            event_id,
            batch_id: None,
            event_ids: vec![event_id],
            key: None,
            account_id,
            workspace_id,
            app_id,
            original_run_id: None,
            replay_id: None,
            priority_factor: None,
        }
    }

    /// Generate the idempotency key for this identifier
    /// This matches the Go implementation's IdempotencyKey() method
    pub fn idempotency_key(&self) -> String {
        match &self.key {
            Some(key) => key.clone(),
            None => {
                let workflow_hash = xxh64(self.workflow_id.as_bytes(), 0);
                let run_hash = xxh64(self.run_id.to_string().as_bytes(), 0);
                format!("{workflow_hash}:{run_hash}")
            }
        }
    }

    /// Check if this is a batch run
    pub fn is_batch(&self) -> bool {
        self.batch_id.is_some()
    }

    /// Check if this is a replay
    pub fn is_replay(&self) -> bool {
        self.replay_id.is_some()
    }

    /// Check if this is a rerun
    pub fn is_rerun(&self) -> bool {
        self.original_run_id.is_some()
    }

    /// Get the effective priority factor (defaults to 0 if not set)
    pub fn effective_priority_factor(&self) -> i64 {
        self.priority_factor.unwrap_or(0)
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "run:{}/fn:{}", self.run_id, self.workflow_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_creation() {
        let run_id = Ulid::new();
        let workflow_id = Uuid::new_v4();
        let event_id = Ulid::new();
        let account_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let app_id = Uuid::new_v4();

        let identifier = Identifier::new(
            run_id,
            workflow_id,
            1,
            event_id,
            account_id,
            workspace_id,
            app_id,
        );

        assert_eq!(identifier.run_id, run_id);
        assert_eq!(identifier.workflow_id, workflow_id);
        assert_eq!(identifier.workflow_version, 1);
        assert_eq!(identifier.event_id, event_id);
        assert_eq!(identifier.event_ids, vec![event_id]);
        assert!(!identifier.is_batch());
        assert!(!identifier.is_replay());
        assert!(!identifier.is_rerun());
    }

    #[test]
    fn test_idempotency_key_default() {
        let identifier = Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            1,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        let key = identifier.idempotency_key();
        assert!(key.contains(':'));
        assert!(!key.is_empty());
    }

    #[test]
    fn test_idempotency_key_custom() {
        let mut identifier = Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            1,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        identifier.key = Some("custom-key".to_string());

        assert_eq!(identifier.idempotency_key(), "custom-key");
    }

    #[test]
    fn test_serialization() {
        let identifier = Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            1,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        let json = serde_json::to_string(&identifier).unwrap();
        let deserialized: Identifier = serde_json::from_str(&json).unwrap();

        assert_eq!(identifier, deserialized);
    }
}
