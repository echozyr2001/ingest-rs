use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents an event that triggers function execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: Option<Uuid>,
    /// Event name/type
    pub name: String,
    /// Event data payload
    pub data: serde_json::Value,
    /// User context data
    pub user: Option<serde_json::Value>,
    /// Event version
    pub version: Option<String>,
    /// Timestamp when event was created
    pub ts: Option<DateTime<Utc>>,
}

/// Internal event with tracking information
#[derive(Debug, Clone)]
pub struct TrackedEvent {
    pub event: Event,
    pub internal_id: Uuid,
    pub workspace_id: Uuid,
    pub received_at: DateTime<Utc>,
}

impl TrackedEvent {
    pub fn new(event: Event, workspace_id: Uuid) -> Self {
        Self {
            event,
            internal_id: Uuid::new_v4(),
            workspace_id,
            received_at: Utc::now(),
        }
    }

    pub fn get_event(&self) -> &Event {
        &self.event
    }

    pub fn get_internal_id(&self) -> Uuid {
        self.internal_id
    }

    pub fn get_workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    /// Convert event to a map for expression evaluation
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert(
            "name".to_string(),
            serde_json::Value::String(self.event.name.clone()),
        );
        map.insert("data".to_string(), self.event.data.clone());

        if let Some(user) = &self.event.user {
            map.insert("user".to_string(), user.clone());
        }

        if let Some(version) = &self.event.version {
            map.insert(
                "version".to_string(),
                serde_json::Value::String(version.clone()),
            );
        }

        if let Some(ts) = &self.event.ts {
            map.insert("ts".to_string(), serde_json::Value::String(ts.to_rfc3339()));
        }

        map
    }
}

/// Event manager trait for handling event operations
#[async_trait::async_trait]
pub trait EventManager {
    type Error;

    /// Send an event to the system
    async fn send(&self, event: Event) -> Result<TrackedEvent, Self::Error>;

    /// Send multiple events
    async fn send_batch(&self, events: Vec<Event>) -> Result<Vec<TrackedEvent>, Self::Error>;

    /// Get event by ID
    async fn get_event(&self, id: Uuid) -> Result<Option<TrackedEvent>, Self::Error>;

    /// Store event for historical purposes
    async fn store_event(&self, tracked_event: &TrackedEvent) -> Result<(), Self::Error>;
}
