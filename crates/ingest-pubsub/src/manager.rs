use crate::{
    InMemoryBroker, Message, MessageBroker, PersistentBroker, PubSubError, Result, Subscriber,
    SubscriberId, Topic, TopicPattern,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Configuration for the PubSub manager
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    /// Whether to use persistent storage
    pub persistent: bool,
    /// Maximum number of topics
    pub max_topics: Option<usize>,
    /// Maximum number of subscriptions
    pub max_subscriptions: Option<usize>,
    /// Message retention duration in seconds
    pub message_retention_seconds: Option<u64>,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl PubSubConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self {
            persistent: false,
            max_topics: None,
            max_subscriptions: None,
            message_retention_seconds: None,
            enable_metrics: true,
        }
    }

    /// Enable persistent storage
    pub fn with_persistence(mut self) -> Self {
        self.persistent = true;
        self
    }

    /// Set maximum number of topics
    pub fn with_max_topics(mut self, max_topics: usize) -> Self {
        self.max_topics = Some(max_topics);
        self
    }

    /// Set maximum number of subscriptions
    pub fn with_max_subscriptions(mut self, max_subscriptions: usize) -> Self {
        self.max_subscriptions = Some(max_subscriptions);
        self
    }

    /// Set message retention duration
    pub fn with_message_retention(mut self, retention_seconds: u64) -> Self {
        self.message_retention_seconds = Some(retention_seconds);
        self
    }

    /// Disable metrics collection
    pub fn without_metrics(mut self) -> Self {
        self.enable_metrics = false;
        self
    }
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Main PubSub manager coordinating all pub-sub operations
pub struct PubSubManager {
    /// Configuration
    config: PubSubConfig,
    /// Message broker
    broker: Arc<dyn MessageBroker>,
    /// Manager state
    state: Arc<RwLock<ManagerState>>,
}

#[derive(Debug)]
struct ManagerState {
    /// Whether the manager is running
    running: bool,
    /// Total messages processed
    total_messages_processed: u64,
    /// Total subscriptions created
    total_subscriptions_created: u64,
}

impl ManagerState {
    fn new() -> Self {
        Self {
            running: false,
            total_messages_processed: 0,
            total_subscriptions_created: 0,
        }
    }
}

impl PubSubManager {
    /// Create a new PubSub manager with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(PubSubConfig::default()).await
    }

    /// Create a new PubSub manager with custom configuration
    pub async fn with_config(config: PubSubConfig) -> Result<Self> {
        let broker: Arc<dyn MessageBroker> = if config.persistent {
            Arc::new(PersistentBroker::new())
        } else {
            Arc::new(InMemoryBroker::new())
        };

        let manager = Self {
            config,
            broker,
            state: Arc::new(RwLock::new(ManagerState::new())),
        };

        info!(
            "Created PubSub manager with persistent storage: {}",
            manager.config.persistent
        );
        Ok(manager)
    }

    /// Start the PubSub manager
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if state.running {
            return Err(PubSubError::invalid_operation(
                "start",
                "Manager is already running",
            ));
        }

        state.running = true;
        info!("Started PubSub manager");
        Ok(())
    }

    /// Stop the PubSub manager
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if !state.running {
            return Err(PubSubError::invalid_operation(
                "stop",
                "Manager is not running",
            ));
        }

        state.running = false;
        info!("Stopped PubSub manager");
        Ok(())
    }

    /// Check if the manager is running
    pub async fn is_running(&self) -> bool {
        let state = self.state.read().await;
        state.running
    }

    /// Get the configuration
    pub fn config(&self) -> &PubSubConfig {
        &self.config
    }

    /// Publish a message to a topic
    pub async fn publish(&self, topic: &Topic, message: Message) -> Result<()> {
        self.check_running().await?;
        self.check_topic_limits().await?;

        let result = self.broker.publish(topic, message).await;

        if result.is_ok() {
            self.update_state(|state| {
                state.total_messages_processed += 1;
            })
            .await;
        }

        result
    }

    /// Publish multiple messages to a topic
    pub async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()> {
        self.check_running().await?;
        self.check_topic_limits().await?;

        let message_count = messages.len();
        let result = self.broker.publish_batch(topic, messages).await;

        if result.is_ok() {
            self.update_state(|state| {
                state.total_messages_processed += message_count as u64;
            })
            .await;
        }

        result
    }

    /// Subscribe to a topic pattern
    pub async fn subscribe(&self, pattern: &TopicPattern) -> Result<Subscriber> {
        self.check_running().await?;
        self.check_subscription_limits().await?;

        let result = self.broker.subscribe(pattern).await;

        if result.is_ok() {
            self.update_state(|state| {
                state.total_subscriptions_created += 1;
            })
            .await;
        }

        result
    }

    /// Unsubscribe a subscriber
    pub async fn unsubscribe(&self, subscriber_id: &SubscriberId) -> Result<bool> {
        self.check_running().await?;
        self.broker.unsubscribe(subscriber_id).await
    }

    /// Get broker statistics
    pub async fn get_broker_stats(&self) -> Result<crate::broker::BrokerStats> {
        self.broker.get_stats().await
    }

    /// Get manager statistics
    pub async fn get_manager_stats(&self) -> Result<ManagerStats> {
        let state = self.state.read().await;
        let broker_stats = self.broker.get_stats().await?;

        Ok(ManagerStats {
            running: state.running,
            total_messages_processed: state.total_messages_processed,
            total_subscriptions_created: state.total_subscriptions_created,
            current_topic_count: broker_stats.topic_count,
            current_subscription_count: broker_stats.subscription_count,
            messages_published: broker_stats.messages_published,
            messages_delivered: broker_stats.messages_delivered,
            failed_deliveries: broker_stats.failed_deliveries,
            delivery_success_rate: broker_stats.delivery_success_rate(),
        })
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<HealthCheckResult> {
        let broker_healthy = self.broker.health_check().await?;
        let state = self.state.read().await;

        Ok(HealthCheckResult {
            manager_running: state.running,
            broker_healthy,
            overall_healthy: state.running && broker_healthy,
        })
    }

    /// Get the underlying broker (for advanced usage)
    pub fn broker(&self) -> &Arc<dyn MessageBroker> {
        &self.broker
    }

    /// Check if the manager is running
    async fn check_running(&self) -> Result<()> {
        let state = self.state.read().await;
        if !state.running {
            return Err(PubSubError::invalid_operation(
                "operation",
                "Manager is not running",
            ));
        }
        Ok(())
    }

    /// Check topic limits
    async fn check_topic_limits(&self) -> Result<()> {
        if let Some(max_topics) = self.config.max_topics {
            let stats = self.broker.get_stats().await?;
            if stats.topic_count >= max_topics {
                return Err(PubSubError::resource_exhausted("topics"));
            }
        }
        Ok(())
    }

    /// Check subscription limits
    async fn check_subscription_limits(&self) -> Result<()> {
        if let Some(max_subscriptions) = self.config.max_subscriptions {
            let stats = self.broker.get_stats().await?;
            if stats.subscription_count >= max_subscriptions {
                return Err(PubSubError::resource_exhausted("subscriptions"));
            }
        }
        Ok(())
    }

    /// Update manager state
    async fn update_state<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut ManagerState),
    {
        let mut state = self.state.write().await;
        update_fn(&mut state);
    }
}

/// Manager statistics
#[derive(Debug, Clone, PartialEq)]
pub struct ManagerStats {
    /// Whether the manager is running
    pub running: bool,
    /// Total messages processed
    pub total_messages_processed: u64,
    /// Total subscriptions created
    pub total_subscriptions_created: u64,
    /// Current number of topics
    pub current_topic_count: usize,
    /// Current number of subscriptions
    pub current_subscription_count: usize,
    /// Total messages published
    pub messages_published: u64,
    /// Total messages delivered
    pub messages_delivered: u64,
    /// Number of failed deliveries
    pub failed_deliveries: u64,
    /// Delivery success rate
    pub delivery_success_rate: f64,
}

/// Health check result
#[derive(Debug, Clone, PartialEq)]
pub struct HealthCheckResult {
    /// Whether the manager is running
    pub manager_running: bool,
    /// Whether the broker is healthy
    pub broker_healthy: bool,
    /// Overall health status
    pub overall_healthy: bool,
}

#[async_trait]
impl crate::traits::Publisher for PubSubManager {
    async fn publish(&self, topic: &Topic, message: Message) -> Result<()> {
        self.publish(topic, message).await
    }

    async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()> {
        self.publish_batch(topic, messages).await
    }

    async fn publish_and_wait(&self, topic: &Topic, message: Message) -> Result<()> {
        // For now, just publish (could be extended for acknowledgments)
        self.publish(topic, message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_pubsub_config_creation() {
        let actual = PubSubConfig::new();

        assert!(!actual.persistent);
        assert!(actual.max_topics.is_none());
        assert!(actual.max_subscriptions.is_none());
        assert!(actual.message_retention_seconds.is_none());
        assert!(actual.enable_metrics);
    }

    #[test]
    fn test_pubsub_config_builder() {
        let actual = PubSubConfig::new()
            .with_persistence()
            .with_max_topics(100)
            .with_max_subscriptions(1000)
            .with_message_retention(3600)
            .without_metrics();

        assert!(actual.persistent);
        assert_eq!(actual.max_topics, Some(100));
        assert_eq!(actual.max_subscriptions, Some(1000));
        assert_eq!(actual.message_retention_seconds, Some(3600));
        assert!(!actual.enable_metrics);
    }

    #[tokio::test]
    async fn test_pubsub_manager_creation() {
        let actual = PubSubManager::new().await.unwrap();

        assert!(!actual.config().persistent);
        assert!(!actual.is_running().await);
    }

    #[tokio::test]
    async fn test_pubsub_manager_with_config() {
        let config = PubSubConfig::new().with_persistence();
        let actual = PubSubManager::with_config(config).await.unwrap();

        assert!(actual.config().persistent);
    }

    #[tokio::test]
    async fn test_pubsub_manager_start_stop() {
        let fixture = PubSubManager::new().await.unwrap();

        // Should not be running initially
        assert!(!fixture.is_running().await);

        // Start the manager
        fixture.start().await.unwrap();
        assert!(fixture.is_running().await);

        // Should fail to start again
        let result = fixture.start().await;
        assert!(result.is_err());

        // Stop the manager
        fixture.stop().await.unwrap();
        assert!(!fixture.is_running().await);

        // Should fail to stop again
        let result = fixture.stop().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pubsub_manager_operations_require_running() {
        let fixture = PubSubManager::new().await.unwrap();
        let topic = Topic::new("test.topic");
        let pattern = TopicPattern::new("test.*");
        let message = Message::new(json!({"test": "data"}));

        // Operations should fail when not running
        let result = fixture.publish(&topic, message.clone()).await;
        assert!(result.is_err());

        let result = fixture.subscribe(&pattern).await;
        assert!(result.is_err());

        // Start manager
        fixture.start().await.unwrap();

        // Operations should succeed when running
        let result = fixture.publish(&topic, message).await;
        assert!(result.is_ok());

        let result = fixture.subscribe(&pattern).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pubsub_manager_publish_and_subscribe() {
        let fixture = PubSubManager::new().await.unwrap();
        fixture.start().await.unwrap();

        let topic = Topic::new("test.topic");
        let pattern = TopicPattern::new("test.*");

        // Subscribe first
        let mut subscriber = fixture.subscribe(&pattern).await.unwrap();

        // Publish message
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));
        fixture.publish(&topic, message.clone()).await.unwrap();

        // Should receive the message
        let received = subscriber.next().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload(), message.payload());
    }

    #[tokio::test]
    async fn test_pubsub_manager_batch_publish() {
        let fixture = PubSubManager::new().await.unwrap();
        fixture.start().await.unwrap();

        let topic = Topic::new("test.topic");
        let pattern = TopicPattern::new("test.*");

        // Subscribe first
        let mut subscriber = fixture.subscribe(&pattern).await.unwrap();

        // Publish batch of messages
        let messages = vec![
            Message::new_with_topic("test.topic", json!({"test": "data1"})),
            Message::new_with_topic("test.topic", json!({"test": "data2"})),
            Message::new_with_topic("test.topic", json!({"test": "data3"})),
        ];
        fixture
            .publish_batch(&topic, messages.clone())
            .await
            .unwrap();

        // Should receive all messages
        for expected_message in messages {
            let received = subscriber.next().await;
            assert!(received.is_some());
            assert_eq!(received.unwrap().payload(), expected_message.payload());
        }
    }

    #[tokio::test]
    async fn test_pubsub_manager_unsubscribe() {
        let fixture = PubSubManager::new().await.unwrap();
        fixture.start().await.unwrap();

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
    async fn test_pubsub_manager_statistics() {
        let fixture = PubSubManager::new().await.unwrap();
        fixture.start().await.unwrap();

        let topic = Topic::new("test.topic");
        let pattern = TopicPattern::new("test.*");

        // Initial stats
        let stats = fixture.get_manager_stats().await.unwrap();
        assert_eq!(stats.total_messages_processed, 0);
        assert_eq!(stats.total_subscriptions_created, 0);

        // Create subscription
        let _subscriber = fixture.subscribe(&pattern).await.unwrap();

        // Publish message
        let message = Message::new_with_topic("test.topic", json!({"test": "data"}));
        fixture.publish(&topic, message).await.unwrap();

        // Check updated stats
        let stats = fixture.get_manager_stats().await.unwrap();
        assert_eq!(stats.total_messages_processed, 1);
        assert_eq!(stats.total_subscriptions_created, 1);
        assert_eq!(stats.current_subscription_count, 1);
        assert_eq!(stats.messages_published, 1);
        assert_eq!(stats.messages_delivered, 1);
    }

    #[tokio::test]
    async fn test_pubsub_manager_health_check() {
        let fixture = PubSubManager::new().await.unwrap();

        // Should be unhealthy when not running
        let health = fixture.health_check().await.unwrap();
        assert!(!health.manager_running);
        assert!(!health.overall_healthy);

        // Should be healthy when running
        fixture.start().await.unwrap();
        let health = fixture.health_check().await.unwrap();
        assert!(health.manager_running);
        assert!(health.broker_healthy);
        assert!(health.overall_healthy);
    }

    #[tokio::test]
    async fn test_pubsub_manager_limits() {
        let config = PubSubConfig::new()
            .with_max_topics(1)
            .with_max_subscriptions(1);
        let fixture = PubSubManager::with_config(config).await.unwrap();
        fixture.start().await.unwrap();

        // Should allow first topic and subscription
        let topic1 = Topic::new("test.topic1");
        let pattern1 = TopicPattern::new("test1.*");

        fixture
            .publish(&topic1, Message::new(json!({})))
            .await
            .unwrap();
        fixture.subscribe(&pattern1).await.unwrap();

        // Should reject additional topic and subscription due to limits
        let topic2 = Topic::new("test.topic2");
        let pattern2 = TopicPattern::new("test2.*");

        let result = fixture.publish(&topic2, Message::new(json!({}))).await;
        assert!(result.is_err());

        let result = fixture.subscribe(&pattern2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pubsub_manager_publisher_trait() {
        use crate::traits::Publisher;

        let fixture = PubSubManager::new().await.unwrap();
        fixture.start().await.unwrap();

        let topic = Topic::new("test.topic");
        let message = Message::new(json!({"test": "data"}));

        // Should work through Publisher trait
        let result = fixture.publish(&topic, message.clone()).await;
        assert!(result.is_ok());

        let result = fixture.publish_and_wait(&topic, message).await;
        assert!(result.is_ok());
    }
}
