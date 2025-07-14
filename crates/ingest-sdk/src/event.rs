//! Event publishing and building utilities

use crate::error::Result;
use ingest_core::{Event, EventId, Json};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// Builder for creating events
#[derive(Debug, Clone)]
pub struct EventBuilder {
    name: String,
    data: Option<Json>,
    user: Option<Json>,
    id: Option<EventId>,
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
    version: Option<String>,
}

impl EventBuilder {
    /// Create a new event builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: None,
            user: None,
            id: None,
            timestamp: None,
            version: None,
        }
    }

    /// Set the event data
    pub fn data(mut self, data: Json) -> Self {
        self.data = Some(data);
        self
    }

    /// Set the event data from a serializable value
    pub fn data_from<T: serde::Serialize>(mut self, data: T) -> Result<Self> {
        self.data = Some(serde_json::to_value(data)?);
        Ok(self)
    }

    /// Set the user context
    pub fn user(mut self, user: Json) -> Self {
        self.user = Some(user);
        self
    }

    /// Set the user context from a serializable value
    pub fn user_from<T: serde::Serialize>(mut self, user: T) -> Result<Self> {
        self.user = Some(serde_json::to_value(user)?);
        Ok(self)
    }

    /// Set a custom event ID
    pub fn id(mut self, id: EventId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the event timestamp
    pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set the event version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Build the event
    #[instrument(skip(self))]
    pub fn build(self) -> Result<Event> {
        debug!("Building event: {}", self.name);

        let data = self
            .data
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let mut event = Event::new(&self.name, data);

        if let Some(id) = self.id {
            event.id = id;
        }

        if let Some(timestamp) = self.timestamp {
            event.timestamp = timestamp;
        }

        if let Some(user) = self.user {
            event.user = Some(serde_json::to_string(&user)?);
        }

        if let Some(version) = self.version {
            event.version = Some(version);
        }

        Ok(event)
    }
}

/// High-level event publisher
pub struct EventPublisher {
    client: crate::client::IngestClient,
}

impl EventPublisher {
    /// Create a new event publisher
    pub fn new(client: crate::client::IngestClient) -> Self {
        Self { client }
    }

    /// Publish a single event using the builder pattern
    #[instrument(skip(self))]
    pub async fn publish(&self, builder: EventBuilder) -> Result<EventId> {
        let event = builder.build()?;
        self.client.publish_event(event).await
    }

    /// Publish multiple events
    #[instrument(skip(self, builders))]
    pub async fn publish_batch(&self, builders: Vec<EventBuilder>) -> Result<Vec<EventId>> {
        let mut events = Vec::with_capacity(builders.len());

        for builder in builders {
            events.push(builder.build()?);
        }

        self.client.publish_events(events).await
    }

    /// Create a new event builder
    pub fn event(&self, name: impl Into<String>) -> EventBuilder {
        EventBuilder::new(name)
    }
}

/// Convenience functions for common event patterns
pub mod patterns {
    use super::*;

    /// Create a user event
    pub fn user_event(
        action: impl Into<String>,
        user_id: impl Into<String>,
        data: Json,
    ) -> EventBuilder {
        EventBuilder::new(format!("user.{}", action.into()))
            .data(data)
            .user(serde_json::json!({ "id": user_id.into() }))
    }

    /// Create a system event
    pub fn system_event(
        component: impl Into<String>,
        action: impl Into<String>,
        data: Json,
    ) -> EventBuilder {
        EventBuilder::new(format!("system.{}.{}", component.into(), action.into())).data(data)
    }

    /// Create a custom event with metadata
    pub fn custom_event(
        name: impl Into<String>,
        data: Json,
        metadata: HashMap<String, String>,
    ) -> EventBuilder {
        let mut event_data = data;
        if let Value::Object(ref mut map) = event_data {
            map.insert(
                "metadata".to_string(),
                serde_json::to_value(metadata).unwrap(),
            );
        }

        EventBuilder::new(name).data(event_data)
    }

    /// Create a workflow event
    pub fn workflow_event(
        workflow_id: impl Into<String>,
        step: impl Into<String>,
        data: Json,
    ) -> EventBuilder {
        EventBuilder::new(format!("workflow.{}", step.into()))
            .data(data)
            .user(serde_json::json!({ "workflow_id": workflow_id.into() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_event_builder_new() {
        let fixture = EventBuilder::new("test.event");
        let actual = fixture.name;
        let expected = "test.event";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_event_builder_data() {
        let fixture = EventBuilder::new("test.event").data(serde_json::json!({"key": "value"}));
        assert!(fixture.data.is_some());
    }

    #[test]
    fn test_event_builder_data_from() {
        #[derive(serde::Serialize)]
        struct TestData {
            key: String,
        }

        let test_data = TestData {
            key: "value".to_string(),
        };

        let fixture = EventBuilder::new("test.event")
            .data_from(test_data)
            .unwrap();
        assert!(fixture.data.is_some());
    }

    #[test]
    fn test_event_builder_build() {
        let fixture = EventBuilder::new("test.event")
            .data(serde_json::json!({"key": "value"}))
            .build()
            .unwrap();

        let actual = fixture.name();
        let expected = "test.event";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_patterns_user_event() {
        let fixture = patterns::user_event(
            "created",
            "user123",
            serde_json::json!({"email": "test@example.com"}),
        );

        let actual = fixture.name;
        let expected = "user.created";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_patterns_system_event() {
        let fixture = patterns::system_event("auth", "login", serde_json::json!({"success": true}));

        let actual = fixture.name;
        let expected = "system.auth.login";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_patterns_workflow_event() {
        let fixture = patterns::workflow_event(
            "workflow123",
            "completed",
            serde_json::json!({"result": "success"}),
        );

        let actual = fixture.name;
        let expected = "workflow.completed";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_patterns_custom_event() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "test".to_string());

        let fixture = patterns::custom_event(
            "custom.event",
            serde_json::json!({"data": "value"}),
            metadata,
        );

        let actual = fixture.name;
        let expected = "custom.event";
        assert_eq!(actual, expected);
    }
}
