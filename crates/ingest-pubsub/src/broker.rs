use crate::{Message, Result, SubscriptionManager, Topic, TopicManager, TopicPattern};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Message broker trait for routing and delivering messages
#[async_trait]
pub trait MessageBroker: Send + Sync {
    /// Publish a message to a topic
    async fn publish(&self, topic: &Topic, message: Message) -> Result<()>;

    /// Publish multiple messages to a topic
    async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()>;

    /// Subscribe to a topic pattern
    async fn subscribe(&self, pattern: &TopicPattern) -> Result<crate::Subscriber>;

    /// Unsubscribe a subscriber
    async fn unsubscribe(&self, subscriber_id: &crate::SubscriberId) -> Result<bool>;

    /// Get broker statistics
    async fn get_stats(&self) -> Result<BrokerStats>;

    /// Health check
    async fn health_check(&self) -> Result<bool>;
}

/// Broker statistics
#[derive(Debug, Clone, PartialEq)]
pub struct BrokerStats {
    /// Number of topics
    pub topic_count: usize,
    /// Number of active subscriptions
    pub subscription_count: usize,
    /// Total messages published
    pub messages_published: u64,
    /// Total messages delivered
    pub messages_delivered: u64,
    /// Number of failed deliveries
    pub failed_deliveries: u64,
}

impl BrokerStats {
    /// Create new broker stats
    pub fn new() -> Self {
        Self {
            topic_count: 0,
            subscription_count: 0,
            messages_published: 0,
            messages_delivered: 0,
            failed_deliveries: 0,
        }
    }

    /// Get delivery success rate
    pub fn delivery_success_rate(&self) -> f64 {
        if self.messages_delivered + self.failed_deliveries == 0 {
            return 1.0;
        }
        self.messages_delivered as f64 / (self.messages_delivered + self.failed_deliveries) as f64
    }
}

impl Default for BrokerStats {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory message broker implementation
#[derive(Debug)]
pub struct InMemoryBroker {
    /// Topic manager
    topic_manager: TopicManager,
    /// Subscription manager
    subscription_manager: SubscriptionManager,
    /// Broker statistics
    stats: Arc<RwLock<BrokerStats>>,
}

impl InMemoryBroker {
    /// Create a new in-memory broker
    pub fn new() -> Self {
        Self {
            topic_manager: TopicManager::new(),
            subscription_manager: SubscriptionManager::new(),
            stats: Arc::new(RwLock::new(BrokerStats::new())),
        }
    }

    /// Get the topic manager
    pub fn topic_manager(&self) -> &TopicManager {
        &self.topic_manager
    }

    /// Get the subscription manager
    pub fn subscription_manager(&self) -> &SubscriptionManager {
        &self.subscription_manager
    }

    /// Update statistics
    async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut BrokerStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut stats);
    }
}

impl Default for InMemoryBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageBroker for InMemoryBroker {
    async fn publish(&self, topic: &Topic, message: Message) -> Result<()> {
        // Ensure topic exists
        self.topic_manager.get_or_create_topic(topic.name()).await?;

        // Deliver message to subscribers
        let delivered_count = self
            .subscription_manager
            .deliver_message(topic.name(), message)
            .await?;

        // Update statistics
        self.update_stats(|stats| {
            stats.messages_published += 1;
            stats.messages_delivered += delivered_count as u64;
        })
        .await;

        info!(
            "Published message to topic '{}', delivered to {} subscribers",
            topic.name(),
            delivered_count
        );
        Ok(())
    }

    async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()> {
        for message in messages {
            self.publish(topic, message).await?;
        }
        Ok(())
    }

    async fn subscribe(&self, pattern: &TopicPattern) -> Result<crate::Subscriber> {
        let subscriber = self.subscription_manager.subscribe(pattern).await?;

        // Update statistics
        self.update_stats(|stats| {
            stats.subscription_count += 1;
        })
        .await;

        info!("Created subscription for pattern '{}'", pattern);
        Ok(subscriber)
    }

    async fn unsubscribe(&self, subscriber_id: &crate::SubscriberId) -> Result<bool> {
        let result = self.subscription_manager.unsubscribe(subscriber_id).await?;

        if result {
            // Update statistics
            self.update_stats(|stats| {
                stats.subscription_count = stats.subscription_count.saturating_sub(1);
            })
            .await;

            info!("Removed subscription with ID {}", subscriber_id);
        }

        Ok(result)
    }

    async fn get_stats(&self) -> Result<BrokerStats> {
        let mut stats = self.stats.read().await.clone();

        // Update current counts
        stats.topic_count = self.topic_manager.topic_count().await?;
        stats.subscription_count = self.subscription_manager.subscription_count().await?;

        Ok(stats)
    }

    async fn health_check(&self) -> Result<bool> {
        // Basic health check - ensure managers are responsive
        let topic_count = self.topic_manager.topic_count().await?;
        let subscription_count = self.subscription_manager.subscription_count().await?;

        debug!(
            "Health check: {} topics, {} subscriptions",
            topic_count, subscription_count
        );
        Ok(true)
    }
}

/// Persistent message broker implementation (using storage backend)
#[derive(Debug)]
pub struct PersistentBroker {
    /// In-memory broker for fast operations
    memory_broker: InMemoryBroker,
    /// Message storage
    message_storage: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    /// Persistence enabled flag
    persistence_enabled: bool,
}

impl PersistentBroker {
    /// Create a new persistent broker
    pub fn new() -> Self {
        Self {
            memory_broker: InMemoryBroker::new(),
            message_storage: Arc::new(RwLock::new(HashMap::new())),
            persistence_enabled: true,
        }
    }

    /// Create a persistent broker with custom configuration
    pub fn with_persistence(persistence_enabled: bool) -> Self {
        Self {
            memory_broker: InMemoryBroker::new(),
            message_storage: Arc::new(RwLock::new(HashMap::new())),
            persistence_enabled,
        }
    }

    /// Enable or disable persistence
    pub fn set_persistence(&mut self, enabled: bool) {
        self.persistence_enabled = enabled;
    }

    /// Check if persistence is enabled
    pub fn is_persistence_enabled(&self) -> bool {
        self.persistence_enabled
    }

    /// Get the underlying memory broker
    pub fn memory_broker(&self) -> &InMemoryBroker {
        &self.memory_broker
    }

    /// Persist a message to storage
    async fn persist_message(&self, topic: &str, message: &Message) -> Result<()> {
        if !self.persistence_enabled {
            return Ok(());
        }

        let mut storage = self.message_storage.write().await;
        storage
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(message.clone());

        debug!("Persisted message to topic '{}'", topic);
        Ok(())
    }

    /// Get persisted messages for a topic
    pub async fn get_persisted_messages(&self, topic: &str) -> Result<Vec<Message>> {
        let storage = self.message_storage.read().await;
        Ok(storage.get(topic).cloned().unwrap_or_default())
    }

    /// Clear persisted messages for a topic
    pub async fn clear_persisted_messages(&self, topic: &str) -> Result<usize> {
        let mut storage = self.message_storage.write().await;
        let count = storage.get(topic).map(|v| v.len()).unwrap_or(0);
        storage.remove(topic);
        info!("Cleared {} persisted messages for topic '{}'", count, topic);
        Ok(count)
    }

    /// Get total number of persisted messages
    pub async fn persisted_message_count(&self) -> Result<usize> {
        let storage = self.message_storage.read().await;
        Ok(storage.values().map(|v| v.len()).sum())
    }
}

impl Default for PersistentBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageBroker for PersistentBroker {
    async fn publish(&self, topic: &Topic, message: Message) -> Result<()> {
        // Persist message if enabled
        if let Err(e) = self.persist_message(topic.name(), &message).await {
            error!("Failed to persist message: {}", e);
            // Continue with delivery even if persistence fails
        }

        // Delegate to memory broker for delivery
        self.memory_broker.publish(topic, message).await
    }

    async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()> {
        // Persist messages if enabled
        if self.persistence_enabled {
            for message in &messages {
                if let Err(e) = self.persist_message(topic.name(), message).await {
                    error!("Failed to persist message: {}", e);
                }
            }
        }

        // Delegate to memory broker for delivery
        self.memory_broker.publish_batch(topic, messages).await
    }

    async fn subscribe(&self, pattern: &TopicPattern) -> Result<crate::Subscriber> {
        self.memory_broker.subscribe(pattern).await
    }

    async fn unsubscribe(&self, subscriber_id: &crate::SubscriberId) -> Result<bool> {
        self.memory_broker.unsubscribe(subscriber_id).await
    }

    async fn get_stats(&self) -> Result<BrokerStats> {
        self.memory_broker.get_stats().await
    }

    async fn health_check(&self) -> Result<bool> {
        self.memory_broker.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_broker_stats_creation() {
        let actual = BrokerStats::new();

        assert_eq!(actual.topic_count, 0);
        assert_eq!(actual.subscription_count, 0);
        assert_eq!(actual.messages_published, 0);
        assert_eq!(actual.messages_delivered, 0);
        assert_eq!(actual.failed_deliveries, 0);
        assert_eq!(actual.delivery_success_rate(), 1.0);
    }

    #[test]
    fn test_broker_stats_delivery_success_rate() {
        let mut fixture = BrokerStats::new();
        fixture.messages_delivered = 80;
        fixture.failed_deliveries = 20;

        let actual = fixture.delivery_success_rate();
        let expected = 0.8;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_in_memory_broker_creation() {
        let actual = InMemoryBroker::new();

        assert_eq!(actual.topic_manager().topic_count().await.unwrap(), 0);
        assert_eq!(
            actual
                .subscription_manager()
                .subscription_count()
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_in_memory_broker_publish() {
        let fixture = InMemoryBroker::new();
        let topic = Topic::new("test.topic");
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));

        let result = fixture.publish(&topic, message).await;
        assert!(result.is_ok());

        // Topic should be created
        assert!(
            fixture
                .topic_manager()
                .topic_exists("test.topic")
                .await
                .unwrap()
        );

        // Stats should be updated
        let stats = fixture.get_stats().await.unwrap();
        assert_eq!(stats.messages_published, 1);
    }

    #[tokio::test]
    async fn test_in_memory_broker_subscribe_and_publish() {
        let fixture = InMemoryBroker::new();
        let topic = Topic::new("test.topic");
        let pattern = TopicPattern::new("test.*");

        // Subscribe to pattern
        let mut subscriber = fixture.subscribe(&pattern).await.unwrap();

        // Publish message
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));
        fixture.publish(&topic, message.clone()).await.unwrap();

        // Subscriber should receive the message
        let received = subscriber.next().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload(), message.payload());

        // Stats should be updated
        let stats = fixture.get_stats().await.unwrap();
        assert_eq!(stats.subscription_count, 1);
        assert_eq!(stats.messages_published, 1);
        assert_eq!(stats.messages_delivered, 1);
    }

    #[tokio::test]
    async fn test_in_memory_broker_unsubscribe() {
        let fixture = InMemoryBroker::new();
        let pattern = TopicPattern::new("test.*");

        // Subscribe
        let subscriber = fixture.subscribe(&pattern).await.unwrap();
        let subscriber_id = subscriber.id().clone();

        // Unsubscribe
        let result = fixture.unsubscribe(&subscriber_id).await.unwrap();
        assert!(result);

        // Should return false for non-existent subscription
        let result = fixture.unsubscribe(&subscriber_id).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_in_memory_broker_health_check() {
        let fixture = InMemoryBroker::new();
        let result = fixture.health_check().await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_persistent_broker_creation() {
        let actual = PersistentBroker::new();

        assert!(actual.is_persistence_enabled());
        assert_eq!(
            actual
                .memory_broker()
                .topic_manager()
                .topic_count()
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_persistent_broker_with_persistence_disabled() {
        let mut fixture = PersistentBroker::with_persistence(false);

        assert!(!fixture.is_persistence_enabled());

        fixture.set_persistence(true);
        assert!(fixture.is_persistence_enabled());
    }

    #[tokio::test]
    async fn test_persistent_broker_message_persistence() {
        let fixture = PersistentBroker::new();
        let topic = Topic::new("test.topic");
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));

        // Publish message
        fixture.publish(&topic, message.clone()).await.unwrap();

        // Check persisted messages
        let persisted = fixture.get_persisted_messages("test.topic").await.unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].payload(), message.payload());

        // Clear persisted messages
        let cleared_count = fixture
            .clear_persisted_messages("test.topic")
            .await
            .unwrap();
        assert_eq!(cleared_count, 1);

        let persisted = fixture.get_persisted_messages("test.topic").await.unwrap();
        assert!(persisted.is_empty());
    }

    #[tokio::test]
    async fn test_persistent_broker_persistence_disabled() {
        let fixture = PersistentBroker::with_persistence(false);
        let topic = Topic::new("test.topic");
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));

        // Publish message
        fixture.publish(&topic, message).await.unwrap();

        // Should not persist when disabled
        let persisted = fixture.get_persisted_messages("test.topic").await.unwrap();
        assert!(persisted.is_empty());
    }

    #[tokio::test]
    async fn test_persistent_broker_persisted_message_count() {
        let fixture = PersistentBroker::new();
        let topic1 = Topic::new("test.topic1");
        let topic2 = Topic::new("test.topic2");

        // Publish messages to different topics
        fixture
            .publish(
                &topic1,
                Message::new_with_topic("test.topic1", json!({"test": "data1"})),
            )
            .await
            .unwrap();
        fixture
            .publish(
                &topic1,
                Message::new_with_topic("test.topic1", json!({"test": "data2"})),
            )
            .await
            .unwrap();
        fixture
            .publish(
                &topic2,
                Message::new_with_topic("test.topic2", json!({"test": "data3"})),
            )
            .await
            .unwrap();

        let count = fixture.persisted_message_count().await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_persistent_broker_delegation() {
        let fixture = PersistentBroker::new();
        let pattern = TopicPattern::new("test.*");

        // Subscribe through persistent broker
        let mut subscriber = fixture.subscribe(&pattern).await.unwrap();

        // Publish message
        let topic = Topic::new("test.topic");
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));
        fixture.publish(&topic, message.clone()).await.unwrap();

        // Should receive message through delegation
        let received = subscriber.next().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload(), message.payload());

        // Stats should work through delegation
        let stats = fixture.get_stats().await.unwrap();
        assert_eq!(stats.messages_published, 1);
        assert_eq!(stats.messages_delivered, 1);

        // Health check should work through delegation
        let health = fixture.health_check().await.unwrap();
        assert!(health);
    }
}
