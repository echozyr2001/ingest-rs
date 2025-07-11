use crate::{MessageId, MessageMetadata, Result};
use chrono::{DateTime, Utc};
use ingest_core::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A message in the pub-sub system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Message metadata
    pub metadata: MessageMetadata,
    /// Message payload
    pub payload: Json,
}

impl Message {
    /// Create a new message
    pub fn new(payload: Json) -> Self {
        Self {
            metadata: MessageMetadata::new(""),
            payload,
        }
    }

    /// Create a new message with topic
    pub fn new_with_topic(topic: impl Into<String>, payload: Json) -> Self {
        Self {
            metadata: MessageMetadata::new(topic),
            payload,
        }
    }

    /// Create a new message with metadata
    pub fn new_with_metadata(metadata: MessageMetadata, payload: Json) -> Self {
        Self { metadata, payload }
    }

    /// Get the message ID
    pub fn id(&self) -> &MessageId {
        &self.metadata.id
    }

    /// Get the topic
    pub fn topic(&self) -> &str {
        &self.metadata.topic
    }

    /// Set the topic
    pub fn set_topic(&mut self, topic: impl Into<String>) {
        self.metadata.topic = topic.into();
    }

    /// Get the payload
    pub fn payload(&self) -> &Json {
        &self.payload
    }

    /// Set the payload
    pub fn set_payload(&mut self, payload: Json) {
        self.payload = payload;
    }

    /// Get creation timestamp
    pub fn created_at(&self) -> DateTime<Utc> {
        self.metadata.created_at
    }

    /// Get expiration timestamp
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.metadata.expires_at
    }

    /// Set expiration timestamp
    pub fn set_expires_at(&mut self, expires_at: Option<DateTime<Utc>>) {
        self.metadata.expires_at = expires_at;
    }

    /// Get priority
    pub fn priority(&self) -> i32 {
        self.metadata.priority
    }

    /// Set priority
    pub fn set_priority(&mut self, priority: i32) {
        self.metadata.priority = priority;
    }

    /// Get delivery attempts
    pub fn delivery_attempts(&self) -> u32 {
        self.metadata.delivery_attempts
    }

    /// Increment delivery attempts
    pub fn increment_delivery_attempts(&mut self) {
        self.metadata.increment_delivery_attempts();
    }

    /// Check if message has expired
    pub fn is_expired(&self) -> bool {
        self.metadata.is_expired()
    }

    /// Check if message can be retried
    pub fn can_retry(&self) -> bool {
        self.metadata.can_retry()
    }

    /// Add a header
    pub fn add_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.add_header(key, value);
    }

    /// Get a header value
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.metadata.get_header(key)
    }

    /// Remove a header
    pub fn remove_header(&mut self, key: &str) -> Option<String> {
        self.metadata.remove_header(key)
    }

    /// Get all headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.metadata.headers
    }

    /// Set correlation ID
    pub fn set_correlation_id(&mut self, correlation_id: Option<String>) {
        self.metadata.correlation_id = correlation_id;
    }

    /// Get correlation ID
    pub fn correlation_id(&self) -> Option<&String> {
        self.metadata.correlation_id.as_ref()
    }

    /// Set source
    pub fn set_source(&mut self, source: Option<String>) {
        self.metadata.source = source;
    }

    /// Get source
    pub fn source(&self) -> Option<&String> {
        self.metadata.source.as_ref()
    }

    /// Get content type
    pub fn content_type(&self) -> &str {
        &self.metadata.content_type
    }

    /// Set content type
    pub fn set_content_type(&mut self, content_type: impl Into<String>) {
        self.metadata.content_type = content_type.into();
    }

    /// Get encoding
    pub fn encoding(&self) -> &str {
        &self.metadata.encoding
    }

    /// Set encoding
    pub fn set_encoding(&mut self, encoding: impl Into<String>) {
        self.metadata.encoding = encoding.into();
    }

    /// Create a copy of the message with a new ID
    pub fn clone_with_new_id(&self) -> Self {
        let mut new_message = self.clone();
        new_message.metadata.id = MessageId::new();
        new_message.metadata.delivery_attempts = 0;
        new_message
    }

    /// Serialize the message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(Into::into)
    }

    /// Deserialize the message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    /// Get the message size in bytes
    pub fn size(&self) -> usize {
        self.to_bytes().map(|b| b.len()).unwrap_or(0)
    }

    /// Check if the message is valid
    pub fn is_valid(&self) -> bool {
        !self.metadata.topic.is_empty() && !self.is_expired()
    }
}

/// Builder for creating messages
#[derive(Debug, Default)]
pub struct MessageBuilder {
    topic: Option<String>,
    payload: Option<Json>,
    priority: Option<i32>,
    expires_at: Option<DateTime<Utc>>,
    headers: HashMap<String, String>,
    correlation_id: Option<String>,
    source: Option<String>,
    content_type: Option<String>,
    encoding: Option<String>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the topic
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Set the payload
    pub fn payload(mut self, payload: Json) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Set the priority
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set the expiration time
    pub fn expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Add a header
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set the correlation ID
    pub fn correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Set the source
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the content type
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Set the encoding
    pub fn encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = Some(encoding.into());
        self
    }

    /// Build the message
    pub fn build(self) -> Result<Message> {
        let topic = self.topic.unwrap_or_default();
        let payload = self.payload.unwrap_or(Json::Null);

        let mut metadata = MessageMetadata::new(topic);

        if let Some(priority) = self.priority {
            metadata.priority = priority;
        }

        if let Some(expires_at) = self.expires_at {
            metadata.expires_at = Some(expires_at);
        }

        metadata.headers = self.headers;
        metadata.correlation_id = self.correlation_id;
        metadata.source = self.source;

        if let Some(content_type) = self.content_type {
            metadata.content_type = content_type;
        }

        if let Some(encoding) = self.encoding {
            metadata.encoding = encoding;
        }

        Ok(Message::new_with_metadata(metadata, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_message_creation() {
        let payload_fixture = json!({"key": "value"});
        let actual = Message::new(payload_fixture.clone());

        assert_eq!(actual.payload(), &payload_fixture);
        assert_eq!(actual.topic(), "");
        assert_eq!(actual.priority(), 0);
        assert_eq!(actual.delivery_attempts(), 0);
        assert!(!actual.is_expired());
        assert!(actual.can_retry());
    }

    #[test]
    fn test_message_creation_with_topic() {
        let topic_fixture = "test.topic";
        let payload_fixture = json!({"key": "value"});
        let actual = Message::new_with_topic(topic_fixture, payload_fixture.clone());

        assert_eq!(actual.topic(), topic_fixture);
        assert_eq!(actual.payload(), &payload_fixture);
    }

    #[test]
    fn test_message_setters() {
        let mut fixture = Message::new(json!({}));

        fixture.set_topic("new.topic");
        assert_eq!(fixture.topic(), "new.topic");

        let new_payload = json!({"new": "data"});
        fixture.set_payload(new_payload.clone());
        assert_eq!(fixture.payload(), &new_payload);

        fixture.set_priority(10);
        assert_eq!(fixture.priority(), 10);

        let expires_at = Utc::now() + chrono::Duration::hours(1);
        fixture.set_expires_at(Some(expires_at));
        assert_eq!(fixture.expires_at(), Some(expires_at));
    }

    #[test]
    fn test_message_headers() {
        let mut fixture = Message::new(json!({}));

        fixture.add_header("key1", "value1");
        fixture.add_header("key2", "value2");

        assert_eq!(fixture.get_header("key1"), Some(&"value1".to_string()));
        assert_eq!(fixture.get_header("key2"), Some(&"value2".to_string()));
        assert_eq!(fixture.get_header("key3"), None);

        let removed = fixture.remove_header("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(fixture.get_header("key1"), None);

        assert_eq!(fixture.headers().len(), 1);
    }

    #[test]
    fn test_message_correlation_and_source() {
        let mut fixture = Message::new(json!({}));

        fixture.set_correlation_id(Some("corr-123".to_string()));
        assert_eq!(fixture.correlation_id(), Some(&"corr-123".to_string()));

        fixture.set_source(Some("api-service".to_string()));
        assert_eq!(fixture.source(), Some(&"api-service".to_string()));
    }

    #[test]
    fn test_message_content_type_and_encoding() {
        let mut fixture = Message::new(json!({}));

        assert_eq!(fixture.content_type(), "application/json");
        assert_eq!(fixture.encoding(), "utf-8");

        fixture.set_content_type("application/xml");
        assert_eq!(fixture.content_type(), "application/xml");

        fixture.set_encoding("utf-16");
        assert_eq!(fixture.encoding(), "utf-16");
    }

    #[test]
    fn test_message_delivery_attempts() {
        let mut fixture = Message::new(json!({}));

        assert_eq!(fixture.delivery_attempts(), 0);
        assert!(fixture.can_retry());

        fixture.increment_delivery_attempts();
        assert_eq!(fixture.delivery_attempts(), 1);
        assert!(fixture.can_retry());

        // Reach max attempts
        fixture.metadata.delivery_attempts = 3;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_message_expiration() {
        let mut fixture = Message::new(json!({}));

        assert!(!fixture.is_expired());

        // Set expiration in the past
        fixture.set_expires_at(Some(Utc::now() - chrono::Duration::hours(1)));
        assert!(fixture.is_expired());

        // Set expiration in the future
        fixture.set_expires_at(Some(Utc::now() + chrono::Duration::hours(1)));
        assert!(!fixture.is_expired());
    }

    #[test]
    fn test_message_clone_with_new_id() {
        let mut fixture = Message::new(json!({"test": "data"}));
        fixture.increment_delivery_attempts();
        let original_id = fixture.id().clone();

        let actual = fixture.clone_with_new_id();

        assert_ne!(actual.id(), &original_id);
        assert_eq!(actual.payload(), fixture.payload());
        assert_eq!(actual.delivery_attempts(), 0);
    }

    #[test]
    fn test_message_serialization() {
        let fixture = Message::new_with_topic("test.topic", json!({"key": "value"}));

        let bytes = fixture.to_bytes().unwrap();
        let actual = Message::from_bytes(&bytes).unwrap();

        assert_eq!(actual.topic(), fixture.topic());
        assert_eq!(actual.payload(), fixture.payload());
        assert_eq!(actual.id(), fixture.id());
    }

    #[test]
    fn test_message_size() {
        let fixture = Message::new(json!({"key": "value"}));
        let actual = fixture.size();
        assert!(actual > 0);
    }

    #[test]
    fn test_message_validity() {
        let mut fixture = Message::new(json!({}));

        // Invalid without topic
        assert!(!fixture.is_valid());

        // Valid with topic
        fixture.set_topic("test.topic");
        assert!(fixture.is_valid());

        // Invalid when expired
        fixture.set_expires_at(Some(Utc::now() - chrono::Duration::hours(1)));
        assert!(!fixture.is_valid());
    }

    #[test]
    fn test_message_builder() {
        let topic_fixture = "test.topic";
        let payload_fixture = json!({"key": "value"});
        let priority_fixture = 5;
        let expires_at_fixture = Utc::now() + chrono::Duration::hours(1);

        let actual = MessageBuilder::new()
            .topic(topic_fixture)
            .payload(payload_fixture.clone())
            .priority(priority_fixture)
            .expires_at(expires_at_fixture)
            .header("custom", "header")
            .correlation_id("corr-123")
            .source("test-service")
            .content_type("application/xml")
            .encoding("utf-16")
            .build()
            .unwrap();

        assert_eq!(actual.topic(), topic_fixture);
        assert_eq!(actual.payload(), &payload_fixture);
        assert_eq!(actual.priority(), priority_fixture);
        assert_eq!(actual.expires_at(), Some(expires_at_fixture));
        assert_eq!(actual.get_header("custom"), Some(&"header".to_string()));
        assert_eq!(actual.correlation_id(), Some(&"corr-123".to_string()));
        assert_eq!(actual.source(), Some(&"test-service".to_string()));
        assert_eq!(actual.content_type(), "application/xml");
        assert_eq!(actual.encoding(), "utf-16");
    }

    #[test]
    fn test_message_builder_defaults() {
        let actual = MessageBuilder::new().build().unwrap();

        assert_eq!(actual.topic(), "");
        assert_eq!(actual.payload(), &Json::Null);
        assert_eq!(actual.priority(), 0);
        assert_eq!(actual.content_type(), "application/json");
        assert_eq!(actual.encoding(), "utf-8");
    }
}
