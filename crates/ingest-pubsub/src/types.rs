use chrono::{DateTime, Utc};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for messages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Create a new message ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for MessageId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<MessageId> for Uuid {
    fn from(id: MessageId) -> Self {
        id.0
    }
}

/// Unique identifier for subscribers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriberId(pub Uuid);

impl SubscriberId {
    /// Create a new subscriber ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SubscriberId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SubscriberId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SubscriberId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<SubscriberId> for Uuid {
    fn from(id: SubscriberId) -> Self {
        id.0
    }
}

/// Message delivery guarantees
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryGuarantee {
    /// At most once delivery (fire and forget)
    AtMostOnce,
    /// At least once delivery (with acknowledgments)
    AtLeastOnce,
    /// Exactly once delivery (with deduplication)
    ExactlyOnce,
}

impl Default for DeliveryGuarantee {
    fn default() -> Self {
        Self::AtLeastOnce
    }
}

/// Message metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct MessageMetadata {
    /// Message ID
    pub id: MessageId,
    /// Topic name
    pub topic: String,
    /// Message creation timestamp
    pub created_at: DateTime<Utc>,
    /// Message expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Message priority (higher = more priority)
    pub priority: i32,
    /// Delivery guarantee
    pub delivery_guarantee: DeliveryGuarantee,
    /// Number of delivery attempts
    pub delivery_attempts: u32,
    /// Maximum delivery attempts
    pub max_delivery_attempts: u32,
    /// Custom headers
    pub headers: HashMap<String, String>,
    /// Message source
    pub source: Option<String>,
    /// Correlation ID for request tracing
    pub correlation_id: Option<String>,
    /// Message content type
    pub content_type: String,
    /// Message encoding
    pub encoding: String,
}

impl MessageMetadata {
    /// Create new message metadata
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            topic: topic.into(),
            created_at: Utc::now(),
            expires_at: None,
            priority: 0,
            delivery_guarantee: DeliveryGuarantee::default(),
            delivery_attempts: 0,
            max_delivery_attempts: 3,
            headers: HashMap::new(),
            source: None,
            correlation_id: None,
            content_type: "application/json".to_string(),
            encoding: "utf-8".to_string(),
        }
    }

    /// Check if message has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if message can be retried
    pub fn can_retry(&self) -> bool {
        self.delivery_attempts < self.max_delivery_attempts
    }

    /// Increment delivery attempts
    pub fn increment_delivery_attempts(&mut self) {
        self.delivery_attempts += 1;
    }

    /// Add a header
    pub fn add_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(key.into(), value.into());
    }

    /// Get a header value
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    /// Remove a header
    pub fn remove_header(&mut self, key: &str) -> Option<String> {
        self.headers.remove(key)
    }
}

/// Topic configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct TopicConfig {
    /// Topic name
    pub name: String,
    /// Topic description
    pub description: Option<String>,
    /// Whether the topic is persistent
    pub persistent: bool,
    /// Maximum number of messages to retain
    pub max_messages: Option<u64>,
    /// Message retention duration in seconds
    pub retention_seconds: Option<u64>,
    /// Default message TTL in seconds
    pub default_ttl_seconds: Option<u64>,
    /// Topic creation timestamp
    pub created_at: DateTime<Utc>,
    /// Topic last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Topic metadata
    pub metadata: HashMap<String, String>,
}

impl TopicConfig {
    /// Create new topic configuration
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            name: name.into(),
            description: None,
            persistent: true,
            max_messages: None,
            retention_seconds: None,
            default_ttl_seconds: None,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Update the last updated timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
        self.touch();
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Remove metadata
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        let result = self.metadata.remove(key);
        if result.is_some() {
            self.touch();
        }
        result
    }
}

/// Subscription configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct SubscriptionConfig {
    /// Subscriber ID
    pub subscriber_id: SubscriberId,
    /// Topic pattern to subscribe to
    pub topic_pattern: String,
    /// Subscription name/description
    pub name: Option<String>,
    /// Whether the subscription is durable
    pub durable: bool,
    /// Maximum number of unacknowledged messages
    pub max_unacked_messages: u32,
    /// Message acknowledgment timeout in seconds
    pub ack_timeout_seconds: u32,
    /// Whether to enable dead letter queue
    pub enable_dead_letter: bool,
    /// Dead letter topic name
    pub dead_letter_topic: Option<String>,
    /// Subscription creation timestamp
    pub created_at: DateTime<Utc>,
    /// Subscription last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Subscription metadata
    pub metadata: HashMap<String, String>,
}

impl SubscriptionConfig {
    /// Create new subscription configuration
    pub fn new(topic_pattern: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            subscriber_id: SubscriberId::new(),
            topic_pattern: topic_pattern.into(),
            name: None,
            durable: true,
            max_unacked_messages: 100,
            ack_timeout_seconds: 30,
            enable_dead_letter: true,
            dead_letter_topic: None,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Update the last updated timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
        self.touch();
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Remove metadata
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        let result = self.metadata.remove(key);
        if result.is_some() {
            self.touch();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_message_id_creation() {
        let actual = MessageId::new();
        assert_ne!(format!("{actual}"), "");
    }

    #[test]
    fn test_message_id_from_uuid() {
        let uuid_fixture = Uuid::new_v4();
        let actual = MessageId::from_uuid(uuid_fixture);
        let expected = uuid_fixture;
        assert_eq!(*actual.as_uuid(), expected);
    }

    #[test]
    fn test_message_id_display() {
        let fixture = MessageId::new();
        let actual = format!("{fixture}");
        let expected = fixture.0.to_string();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_subscriber_id_creation() {
        let actual = SubscriberId::new();
        assert_ne!(format!("{actual}"), "");
    }

    #[test]
    fn test_subscriber_id_from_uuid() {
        let uuid_fixture = Uuid::new_v4();
        let actual = SubscriberId::from_uuid(uuid_fixture);
        let expected = uuid_fixture;
        assert_eq!(*actual.as_uuid(), expected);
    }

    #[test]
    fn test_delivery_guarantee_default() {
        let actual = DeliveryGuarantee::default();
        let expected = DeliveryGuarantee::AtLeastOnce;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_message_metadata_creation() {
        let topic_fixture = "test.topic";
        let actual = MessageMetadata::new(topic_fixture);

        assert_eq!(actual.topic, topic_fixture);
        assert_eq!(actual.priority, 0);
        assert_eq!(actual.delivery_attempts, 0);
        assert_eq!(actual.max_delivery_attempts, 3);
        assert!(!actual.is_expired());
        assert!(actual.can_retry());
    }

    #[test]
    fn test_message_metadata_expiration() {
        let mut fixture = MessageMetadata::new("test.topic");

        // Not expired initially
        assert!(!fixture.is_expired());

        // Set expiration in the past
        fixture.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(fixture.is_expired());

        // Set expiration in the future
        fixture.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!fixture.is_expired());
    }

    #[test]
    fn test_message_metadata_retry_logic() {
        let mut fixture = MessageMetadata::new("test.topic");

        // Can retry initially
        assert!(fixture.can_retry());

        // Increment attempts
        fixture.increment_delivery_attempts();
        assert_eq!(fixture.delivery_attempts, 1);
        assert!(fixture.can_retry());

        // Reach max attempts
        fixture.delivery_attempts = 3;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_message_metadata_headers() {
        let mut fixture = MessageMetadata::new("test.topic");

        fixture.add_header("key1", "value1");
        fixture.add_header("key2", "value2");

        assert_eq!(fixture.get_header("key1"), Some(&"value1".to_string()));
        assert_eq!(fixture.get_header("key2"), Some(&"value2".to_string()));
        assert_eq!(fixture.get_header("key3"), None);

        let removed = fixture.remove_header("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(fixture.get_header("key1"), None);
    }

    #[test]
    fn test_topic_config_creation() {
        let name_fixture = "test.topic";
        let actual = TopicConfig::new(name_fixture);

        assert_eq!(actual.name, name_fixture);
        assert!(actual.persistent);
        assert!(actual.max_messages.is_none());
        assert!(actual.retention_seconds.is_none());
    }

    #[test]
    fn test_topic_config_metadata() {
        let mut fixture = TopicConfig::new("test.topic");
        let original_updated_at = fixture.updated_at;

        // Add metadata should update timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));
        fixture.add_metadata("key1", "value1");
        assert!(fixture.updated_at > original_updated_at);
        assert_eq!(fixture.get_metadata("key1"), Some(&"value1".to_string()));

        // Remove metadata should update timestamp
        let updated_at_after_add = fixture.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        let removed = fixture.remove_metadata("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert!(fixture.updated_at > updated_at_after_add);
        assert_eq!(fixture.get_metadata("key1"), None);
    }

    #[test]
    fn test_subscription_config_creation() {
        let pattern_fixture = "test.*";
        let actual = SubscriptionConfig::new(pattern_fixture);

        assert_eq!(actual.topic_pattern, pattern_fixture);
        assert!(actual.durable);
        assert_eq!(actual.max_unacked_messages, 100);
        assert_eq!(actual.ack_timeout_seconds, 30);
        assert!(actual.enable_dead_letter);
    }

    #[test]
    fn test_subscription_config_metadata() {
        let mut fixture = SubscriptionConfig::new("test.*");
        let original_updated_at = fixture.updated_at;

        // Add metadata should update timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));
        fixture.add_metadata("key1", "value1");
        assert!(fixture.updated_at > original_updated_at);
        assert_eq!(fixture.get_metadata("key1"), Some(&"value1".to_string()));

        // Remove metadata should update timestamp
        let updated_at_after_add = fixture.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        let removed = fixture.remove_metadata("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert!(fixture.updated_at > updated_at_after_add);
        assert_eq!(fixture.get_metadata("key1"), None);
    }

    #[test]
    fn test_types_serialization() {
        let message_id = MessageId::new();
        let serialized = serde_json::to_string(&message_id).unwrap();
        let deserialized: MessageId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(message_id, deserialized);

        let subscriber_id = SubscriberId::new();
        let serialized = serde_json::to_string(&subscriber_id).unwrap();
        let deserialized: SubscriberId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(subscriber_id, deserialized);

        let delivery_guarantee = DeliveryGuarantee::AtLeastOnce;
        let serialized = serde_json::to_string(&delivery_guarantee).unwrap();
        let deserialized: DeliveryGuarantee = serde_json::from_str(&serialized).unwrap();
        assert_eq!(delivery_guarantee, deserialized);
    }
}
