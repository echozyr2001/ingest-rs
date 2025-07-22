//! Event types and handling
//!
//! This module defines the core Event type and related functionality
//! for handling events in the Inngest system.

use crate::error::{Error, EventError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;
use uuid::Uuid;

/// Maximum event size in bytes (1MB)
pub const MAX_EVENT_SIZE: usize = 1024 * 1024;

/// Maximum number of events in a batch
pub const MAX_EVENTS_PER_BATCH: usize = 50;

/// Standard event structure for Inngest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event name/type
    pub name: String,

    /// Event payload data
    pub data: serde_json::Value,

    /// User-specific data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<serde_json::Value>,

    /// Event ID (optional - if not provided, one will be generated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// ULID for the event (used by execution system)
    #[serde(default = "generate_ulid")]
    pub ulid: Ulid,

    /// Event timestamp in milliseconds (optional - defaults to current time)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,

    /// Event version (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Event with internal tracking information
pub trait TrackedEvent: Send + Sync {
    /// Get the workspace ID for this event
    fn workspace_id(&self) -> Uuid;

    /// Get the internal ID for this event
    fn internal_id(&self) -> Ulid;

    /// Get the underlying event
    fn event(&self) -> &Event;
}

/// Internal event metadata for invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InngestMetadata {
    /// Source app ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_id: Option<String>,

    /// Source function ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fn_id: Option<String>,

    /// Source function version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fn_version: Option<i32>,

    /// Target function ID for invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoke_fn_id: Option<String>,

    /// Correlation ID for invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,

    /// Expiration time for invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,

    /// Group ID for parallel execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,

    /// Display name for the invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Standard implementation of TrackedEvent for development/testing
#[derive(Debug, Clone)]
pub struct StandardTrackedEvent {
    pub internal_id: Ulid,
    pub workspace_id: Uuid,
    pub event: Event,
}

impl TrackedEvent for StandardTrackedEvent {
    fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    fn internal_id(&self) -> Ulid {
        self.internal_id
    }

    fn event(&self) -> &Event {
        &self.event
    }
}

impl Event {
    /// Create a new event with the given name and data
    pub fn new(name: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            data,
            user: None,
            id: None,
            ulid: Ulid::new(),
            timestamp: None,
            version: None,
        }
    }

    /// Set the event ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the user data
    pub fn with_user(mut self, user: serde_json::Value) -> Self {
        self.user = Some(user);
        self
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp.timestamp_millis());
        self
    }

    /// Set the version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Get the event time as a DateTime
    pub fn time(&self) -> DateTime<Utc> {
        match self.timestamp {
            Some(ts) => DateTime::from_timestamp_millis(ts).unwrap_or_else(Utc::now),
            None => Utc::now(),
        }
    }

    /// Convert the event to a map representation
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();

        map.insert(
            "name".to_string(),
            serde_json::Value::String(self.name.clone()),
        );
        map.insert("data".to_string(), self.data.clone());

        if let Some(user) = &self.user {
            map.insert("user".to_string(), user.clone());
        } else {
            map.insert(
                "user".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }

        if let Some(id) = &self.id {
            map.insert("id".to_string(), serde_json::Value::String(id.clone()));
        }

        if let Some(ts) = self.timestamp {
            map.insert(
                "ts".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts)),
            );
        }

        if let Some(version) = &self.version {
            map.insert("v".to_string(), serde_json::Value::String(version.clone()));
        }

        map
    }

    /// Validate the event structure and contents
    pub fn validate(&self) -> Result<()> {
        // Check event name
        if self.name.is_empty() {
            return Err(Error::Event(EventError::MissingField("name".to_string())));
        }

        // Check for reserved internal event names
        if self.name.starts_with("inngest/") {
            return Err(Error::Event(EventError::InvalidName(
                "Event names starting with 'inngest/' are reserved".to_string(),
            )));
        }

        // Validate timestamp if provided
        if let Some(ts) = self.timestamp {
            let start_timestamp = DateTime::from_timestamp(631152000, 0).unwrap(); // 1990-01-01
            let end_timestamp = DateTime::from_timestamp(4102444800, 0).unwrap(); // 2100-01-01
            let event_time = DateTime::from_timestamp_millis(ts).unwrap_or_else(Utc::now);

            if event_time < start_timestamp {
                return Err(Error::Event(EventError::Validation(
                    "Event timestamp cannot be before 1990-01-01".to_string(),
                )));
            }

            if event_time > end_timestamp {
                return Err(Error::Event(EventError::Validation(
                    "Event timestamp cannot be after 2100-01-01".to_string(),
                )));
            }
        }

        // Check serialized size
        let serialized = serde_json::to_vec(self).map_err(Error::Serialization)?;
        if serialized.len() > MAX_EVENT_SIZE {
            return Err(Error::Event(EventError::TooLarge {
                size: serialized.len(),
                max: MAX_EVENT_SIZE,
            }));
        }

        Ok(())
    }

    /// Check if this is an internal event
    pub fn is_internal(&self) -> bool {
        self.name.starts_with("inngest/")
    }

    /// Check if this is a function finished event
    pub fn is_finished_event(&self) -> bool {
        self.name == "inngest/function.finished"
    }

    /// Check if this is an invocation event
    pub fn is_invocation_event(&self) -> bool {
        self.name == "inngest/function.invoke"
    }

    /// Check if this is a cron event
    pub fn is_cron_event(&self) -> bool {
        self.name == "inngest/scheduled.timer"
    }

    /// Get the correlation ID if this is an invocation/finished event
    pub fn correlation_id(&self) -> Option<String> {
        if self.is_invocation_event() {
            if let Ok(metadata) = self.inngest_metadata() {
                return metadata.correlation_id;
            }
        }

        if self.is_finished_event() {
            if let Some(data) = self.data.as_object() {
                if let Some(corr_id) = data.get("correlation_id") {
                    return corr_id.as_str().map(|s| s.to_string());
                }
            }
        }

        None
    }

    /// Extract Inngest metadata from the event data
    pub fn inngest_metadata(&self) -> Result<InngestMetadata> {
        let data = self.data.as_object().ok_or_else(|| {
            Error::Event(EventError::InvalidFormat(
                "Event data must be an object".to_string(),
            ))
        })?;

        let metadata = data
            .get("_inngest")
            .ok_or_else(|| Error::Event(EventError::MissingField("_inngest".to_string())))?;

        serde_json::from_value(metadata.clone()).map_err(|e| {
            Error::Event(EventError::InvalidFormat(format!(
                "Invalid Inngest metadata: {}",
                e
            )))
        })
    }
}

impl StandardTrackedEvent {
    /// Create a new tracked event
    pub fn new(event: Event, workspace_id: Uuid) -> Self {
        let internal_id = Ulid::new();
        Self {
            internal_id,
            workspace_id,
            event,
        }
    }

    /// Create a tracked event with a specific internal ID
    pub fn with_internal_id(event: Event, workspace_id: Uuid, internal_id: Ulid) -> Self {
        Self {
            internal_id,
            workspace_id,
            event,
        }
    }
}

/// Batch of events for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<Event>,
    pub batch_id: Option<String>,
}

impl EventBatch {
    /// Create a new event batch
    pub fn new(events: Vec<Event>) -> Result<Self> {
        if events.len() > MAX_EVENTS_PER_BATCH {
            return Err(Error::Event(EventError::TooLarge {
                size: events.len(),
                max: MAX_EVENTS_PER_BATCH,
            }));
        }

        Ok(Self {
            events,
            batch_id: Some(Ulid::new().to_string()),
        })
    }

    /// Validate all events in the batch
    pub fn validate(&self) -> Result<()> {
        for event in &self.events {
            event.validate()?;
        }
        Ok(())
    }
}

/// Generate a new ULID
fn generate_ulid() -> Ulid {
    Ulid::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_creation() {
        let event = Event::new("test.event", json!({"key": "value"}))
            .with_id("test-id")
            .with_version("2023-05-15");

        assert_eq!(event.name, "test.event");
        assert_eq!(event.id, Some("test-id".to_string()));
        assert_eq!(event.version, Some("2023-05-15".to_string()));
    }

    #[test]
    fn test_event_validation() {
        // Valid event
        let event = Event::new("user.created", json!({"user_id": 123}));
        assert!(event.validate().is_ok());

        // Empty name
        let invalid_event = Event {
            name: "".to_string(),
            data: json!({}),
            user: None,
            id: None,
            ulid: Ulid::new(),
            timestamp: None,
            version: None,
        };
        assert!(invalid_event.validate().is_err());

        // Reserved name
        let reserved_event = Event::new("inngest/internal", json!({}));
        assert!(reserved_event.validate().is_err());
    }

    #[test]
    fn test_event_types() {
        let finished_event = Event::new("inngest/function.finished", json!({}));
        assert!(finished_event.is_finished_event());
        assert!(finished_event.is_internal());

        let user_event = Event::new("user.created", json!({}));
        assert!(!user_event.is_internal());
        assert!(!user_event.is_finished_event());
    }

    #[test]
    fn test_tracked_event() {
        let event = Event::new("test.event", json!({"key": "value"}));
        let workspace_id = Uuid::new_v4();
        let tracked = StandardTrackedEvent::new(event.clone(), workspace_id);

        assert_eq!(tracked.workspace_id(), workspace_id);
        assert_eq!(tracked.event().name, "test.event");
    }

    #[test]
    fn test_event_batch() {
        let events = vec![
            Event::new("event1", json!({})),
            Event::new("event2", json!({})),
        ];

        let batch = EventBatch::new(events).unwrap();
        assert_eq!(batch.events.len(), 2);
        assert!(batch.batch_id.is_some());
        assert!(batch.validate().is_ok());
    }
}
