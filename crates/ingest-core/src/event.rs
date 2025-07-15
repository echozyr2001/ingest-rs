use crate::{DateTime, Id, Json, Result};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Index;

/// Event identifier type
pub type EventId = Id;

/// Event data payload
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct EventData {
    /// The event payload as JSON
    pub data: Json,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Index<&str> for EventData {
    type Output = Json;

    fn index(&self, key: &str) -> &Self::Output {
        &self.data[key]
    }
}

impl EventData {
    /// Create new event data
    pub fn new(data: Json) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
        }
    }

    /// Create event data with metadata
    pub fn with_metadata(data: Json, metadata: HashMap<String, String>) -> Self {
        Self { data, metadata }
    }

    /// Add metadata entry
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Core event type representing an event in the system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct Event {
    /// Unique event identifier
    pub id: EventId,
    /// Event name/type
    #[setters(skip)]
    pub name: String,
    /// Event payload and metadata
    #[setters(skip)]
    pub data: EventData,
    /// Event timestamp
    pub timestamp: DateTime,
    /// Source of the event
    pub source: Option<String>,
    /// Event version for schema evolution
    pub version: Option<String>,
    /// User or system that triggered the event
    pub user: Option<String>,
    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
}

impl Event {
    /// Create a new event
    pub fn new(name: impl Into<String>, data: Json) -> Self {
        Self {
            id: crate::generate_id_with_prefix("evt"),
            name: name.into(),
            data: EventData::new(data),
            timestamp: chrono::Utc::now(),
            source: None,
            version: None,
            user: None,
            trace_id: None,
        }
    }

    /// Create an event with full data
    pub fn with_data(name: impl Into<String>, data: EventData) -> Self {
        Self {
            id: crate::generate_id_with_prefix("evt"),
            name: name.into(),
            data,
            timestamp: chrono::Utc::now(),
            source: None,
            version: None,
            user: None,
            trace_id: None,
        }
    }

    /// Get event name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get event data
    pub fn data(&self) -> &EventData {
        &self.data
    }

    /// Get event payload
    pub fn payload(&self) -> &Json {
        &self.data.data
    }

    /// Check if event matches a pattern
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        // Simple glob-like matching for now
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return self.name.starts_with(prefix);
        }

        self.name == pattern
    }

    /// Validate the event
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(crate::Error::event("Event name cannot be empty"));
        }

        if self.name.len() > 255 {
            return Err(crate::Error::event(
                "Event name too long (max 255 characters)",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event_data() -> EventData {
        EventData::new(json!({
            "user_id": "123",
            "action": "login"
        }))
    }

    #[test]
    fn test_event_data_creation() {
        let fixture = json!({"key": "value"});
        let actual = EventData::new(fixture.clone());
        let expected = EventData {
            data: fixture,
            metadata: HashMap::new(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_event_data_with_metadata() {
        let fixture_data = json!({"key": "value"});
        let mut fixture_metadata = HashMap::new();
        fixture_metadata.insert("source".to_string(), "test".to_string());

        let actual = EventData::with_metadata(fixture_data.clone(), fixture_metadata.clone());
        let expected = EventData {
            data: fixture_data,
            metadata: fixture_metadata,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_event_creation() {
        let fixture_name = "user.login";
        let fixture_data = json!({"user_id": "123"});

        let actual = Event::new(fixture_name, fixture_data.clone());

        assert_eq!(actual.name, "user.login");
        assert_eq!(actual.data.data, fixture_data);
        assert!(actual.id.as_str().starts_with("evt_"));
    }

    #[test]
    fn test_event_with_data() {
        let fixture_name = "user.login";
        let fixture_event_data = create_test_event_data();

        let actual = Event::with_data(fixture_name, fixture_event_data.clone());
        let expected_name = "user.login";
        let expected_data = fixture_event_data;

        assert_eq!(actual.name, expected_name);
        assert_eq!(actual.data, expected_data);
    }

    #[test]
    fn test_event_setters() {
        let mut fixture = Event::new("test.event", json!({}));
        fixture.source = Some("api".to_string());
        fixture.version = Some("1.0".to_string());
        fixture.user = Some("user123".to_string());
        fixture.trace_id = Some("trace123".to_string());

        assert_eq!(fixture.source, Some("api".to_string()));
        assert_eq!(fixture.version, Some("1.0".to_string()));
        assert_eq!(fixture.user, Some("user123".to_string()));
        assert_eq!(fixture.trace_id, Some("trace123".to_string()));
    }

    #[test]
    fn test_event_matches_pattern() {
        let fixture = Event::new("user.login", json!({}));

        assert!(fixture.matches_pattern("*"));
        assert!(fixture.matches_pattern("user.*"));
        assert!(fixture.matches_pattern("user.login"));
        assert!(!fixture.matches_pattern("user.logout"));
        assert!(!fixture.matches_pattern("admin.*"));
    }

    #[test]
    fn test_event_validation_success() {
        let fixture = Event::new("valid.event", json!({}));
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_event_validation_empty_name() {
        let fixture = Event::new("", json!({}));
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_event_validation_long_name() {
        let fixture = Event::new("a".repeat(256), json!({}));
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_event_serialization() {
        let fixture = Event::new("test.event", json!({"key": "value"}));
        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_event_deserialization() {
        let fixture = Event::new("test.event", json!({"key": "value"}));
        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.name, fixture.name);
        assert_eq!(actual.data, fixture.data);
    }

    #[test]
    fn test_event_data_setters() {
        let fixture_data = json!({"key": "value"});
        let actual = EventData::new(fixture_data.clone())
            .data(json!({"updated": "value"}))
            .metadata(HashMap::from([("source".to_string(), "test".to_string())]));

        assert_eq!(actual.data, json!({"updated": "value"}));
        assert_eq!(actual.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_event_setters_with_derive() {
        let fixture = Event::new("test.event", json!({}))
            .source("api")
            .version("1.0")
            .user("user123")
            .trace_id("trace123");

        assert_eq!(fixture.source, Some("api".to_string()));
        assert_eq!(fixture.version, Some("1.0".to_string()));
        assert_eq!(fixture.user, Some("user123".to_string()));
        assert_eq!(fixture.trace_id, Some("trace123".to_string()));
    }
}
