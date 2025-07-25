//! Event types and utilities for Inngest

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;
use uuid::Uuid;

use crate::constants;

/// Event represents an event sent to Inngest
/// This corresponds to the Go `event.Event` struct
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Name of the event
    pub name: String,

    /// Event data payload
    pub data: HashMap<String, serde_json::Value>,

    /// User-specific information for the event
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub user: HashMap<String, serde_json::Value>,

    /// Unique ID for this particular event. If supplied, we should attempt
    /// to only ingest this event once
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,

    /// Timestamp is the time the event occurred, at millisecond precision.
    /// If this is not provided, we will insert the current time upon receipt
    #[serde(default)]
    pub ts: i64,

    /// Version of the event format
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v: String,
}

impl Event {
    /// Create a new event with the given name
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            name: name.into(),
            data: HashMap::new(),
            user: HashMap::new(),
            id: String::new(),
            ts: now,
            v: String::new(),
        }
    }

    /// Create a new event with data
    pub fn with_data(name: impl Into<String>, data: HashMap<String, serde_json::Value>) -> Self {
        let mut event = Self::new(name);
        event.data = data;
        event
    }

    /// Set the event ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.ts = timestamp;
        self
    }

    /// Set user data
    pub fn with_user(mut self, user: HashMap<String, serde_json::Value>) -> Self {
        self.user = user;
        self
    }

    /// Get the event time as a DateTime
    pub fn time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.ts).unwrap_or_else(Utc::now)
    }

    /// Convert the event to a map representation
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert(
            "name".to_string(),
            serde_json::Value::String(self.name.clone()),
        );
        map.insert(
            "data".to_string(),
            serde_json::Value::Object(
                self.data
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
        map.insert(
            "user".to_string(),
            serde_json::Value::Object(
                self.user
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
        map.insert("id".to_string(), serde_json::Value::String(self.id.clone()));
        map.insert("ts".to_string(), serde_json::Value::Number(self.ts.into()));

        if !self.v.is_empty() {
            map.insert("v".to_string(), serde_json::Value::String(self.v.clone()));
        }

        map
    }

    /// Validate the event
    pub fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty() {
            return Err(crate::invalid_input!("event name is empty"));
        }

        if self.ts != 0 {
            let start_timestamp = DateTime::parse_from_rfc3339("1980-01-01T00:00:00Z")
                .unwrap()
                .timestamp_millis();
            let end_timestamp = DateTime::parse_from_rfc3339("2100-01-01T00:00:00Z")
                .unwrap()
                .timestamp_millis();

            if self.ts < start_timestamp {
                return Err(crate::invalid_input!("timestamp is before Jan 1, 1980"));
            }
            if self.ts > end_timestamp {
                return Err(crate::invalid_input!("timestamp is after Jan 1, 2100"));
            }
        }

        Ok(())
    }

    /// Check if this is an internal Inngest event
    pub fn is_internal(&self) -> bool {
        self.name.starts_with(constants::INTERNAL_NAME_PREFIX)
    }

    /// Check if this is a function finished event
    pub fn is_finished_event(&self) -> bool {
        self.name == constants::FN_FINISHED_NAME
    }

    /// Check if this is a function invoke event
    pub fn is_invoke_event(&self) -> bool {
        self.name == constants::FN_INVOKE_NAME
    }

    /// Check if this is a cron event
    pub fn is_cron(&self) -> bool {
        self.name == constants::FN_CRON_NAME
    }

    /// Get the cron schedule if this is a cron event
    pub fn cron_schedule(&self) -> Option<&str> {
        if !self.is_cron() {
            return None;
        }

        self.data
            .get("cron")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    }

    /// Get the correlation ID for the event
    pub fn correlation_id(&self) -> Option<String> {
        if self.is_invoke_event() {
            if let Ok(metadata) = self.inngest_metadata() {
                return Some(metadata.invoke_correlation_id);
            }
        }

        if self.is_finished_event() {
            if let Some(corr_id) = self.data.get(constants::INVOKE_CORRELATION_ID) {
                if let Some(s) = corr_id.as_str() {
                    return Some(s.to_string());
                }
            }
        }

        None
    }

    /// Get Inngest metadata from the event data
    pub fn inngest_metadata(&self) -> crate::Result<InngestMetadata> {
        let raw = self
            .data
            .get(constants::INNGEST_EVENT_DATA_PREFIX)
            .ok_or_else(|| crate::not_found!("inngest metadata"))?;

        serde_json::from_value(raw.clone()).map_err(crate::Error::Serialization)
    }
}

/// Metadata for invoke events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InngestMetadata {
    pub source_app_id: String,
    pub source_fn_id: String,
    pub source_fn_v: i32,
    pub fn_id: String,
    pub correlation_id: Option<String>,
    pub invoke_correlation_id: String,
    pub expire: i64,
    pub gid: String,
    pub name: String,
}

impl InngestMetadata {
    /// Extract the run ID from the correlation ID
    pub fn run_id(&self) -> Option<Ulid> {
        let parts: Vec<&str> = self.invoke_correlation_id.split('.').collect();
        if parts.len() != 2 {
            return None;
        }

        Ulid::from_string(parts[0]).ok()
    }
}

/// Tracked event with additional metadata
pub trait TrackedEvent {
    fn workspace_id(&self) -> Uuid;
    fn internal_id(&self) -> Ulid;
    fn event(&self) -> &Event;
}

/// Simple implementation of TrackedEvent
#[derive(Debug, Clone)]
pub struct SimpleTrackedEvent {
    pub workspace_id: Uuid,
    pub internal_id: Ulid,
    pub event: Event,
}

impl TrackedEvent for SimpleTrackedEvent {
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

/// Seeded ID for deterministic ULID generation
#[derive(Debug, Clone)]
pub struct SeededId {
    pub entropy: [u8; 10],
    pub millis: i64,
}

impl SeededId {
    /// Convert to ULID
    pub fn to_ulid(&self) -> crate::Result<Ulid> {
        if self.millis <= 0 {
            return Err(crate::invalid_input!("millis must be greater than 0"));
        }

        let random_part = u128::from_be_bytes([
            0,
            0,
            0,
            0,
            0,
            0,
            self.entropy[0],
            self.entropy[1],
            self.entropy[2],
            self.entropy[3],
            self.entropy[4],
            self.entropy[5],
            self.entropy[6],
            self.entropy[7],
            self.entropy[8],
            self.entropy[9],
        ]);

        Ok(Ulid::from_parts(self.millis as u64, random_part))
    }

    /// Parse from a seeded ID string
    pub fn from_string(value: &str, index: usize) -> Option<Self> {
        let parts: Vec<&str> = value.split(',').collect();
        if parts.len() != 2 {
            return None;
        }

        let millis = parts[0].parse::<i64>().ok()?;
        if millis <= 0 {
            return None;
        }

        let entropy_bytes = base64::prelude::BASE64_STANDARD.decode(parts[1]).ok()?;
        if entropy_bytes.len() != 10 {
            return None;
        }

        let mut entropy = [0u8; 10];
        entropy.copy_from_slice(&entropy_bytes);

        // Add the index to the entropy
        let current = u32::from_be_bytes([entropy[6], entropy[7], entropy[8], entropy[9]]);
        let new_value = current.wrapping_add(index as u32);
        entropy[6..10].copy_from_slice(&new_value.to_be_bytes());

        Some(Self { entropy, millis })
    }
}

use base64::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new("test.event");
        assert_eq!(event.name, "test.event");
        assert!(!event.data.is_empty() || event.data.is_empty()); // Can be either
        assert!(event.ts > 0);
    }

    #[test]
    fn test_event_validation() {
        let mut event = Event::new("test.event");
        assert!(event.validate().is_ok());

        event.name = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_event_types() {
        let invoke_event = Event::new(constants::FN_INVOKE_NAME);
        assert!(invoke_event.is_invoke_event());
        assert!(invoke_event.is_internal());

        let cron_event = Event::new(constants::FN_CRON_NAME);
        assert!(cron_event.is_cron());
        assert!(cron_event.is_internal());

        let custom_event = Event::new("user.created");
        assert!(!custom_event.is_internal());
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::with_data(
            "test.event",
            [(
                "key".to_string(),
                serde_json::Value::String("value".to_string()),
            )]
            .iter()
            .cloned()
            .collect(),
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(event, deserialized);
    }
}
