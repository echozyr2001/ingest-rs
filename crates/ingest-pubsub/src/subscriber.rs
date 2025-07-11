use crate::{Message, PubSubError, Result, SubscriberId, SubscriptionConfig, TopicPattern};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

/// Message receiver for consuming messages
pub type MessageReceiver = mpsc::UnboundedReceiver<Message>;

/// Message sender for publishing messages to subscribers
pub type MessageSender = mpsc::UnboundedSender<Message>;

/// A subscriber handle for managing subscriptions
#[derive(Debug)]
pub struct SubscriberHandle {
    /// Subscriber ID
    pub id: SubscriberId,
    /// Subscription configuration
    pub config: SubscriptionConfig,
    /// Message sender channel
    sender: MessageSender,
}

impl SubscriberHandle {
    /// Create a new subscriber handle
    pub fn new(config: SubscriptionConfig) -> (Self, MessageReceiver) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let handle = Self {
            id: config.subscriber_id.clone(),
            config,
            sender,
        };
        (handle, receiver)
    }

    /// Get the subscriber ID
    pub fn id(&self) -> &SubscriberId {
        &self.id
    }

    /// Get the subscription configuration
    pub fn config(&self) -> &SubscriptionConfig {
        &self.config
    }

    /// Get the topic pattern
    pub fn topic_pattern(&self) -> &str {
        &self.config.topic_pattern
    }

    /// Send a message to the subscriber
    pub async fn send_message(&self, message: Message) -> Result<()> {
        self.sender.send(message).map_err(|_| {
            PubSubError::subscription("Failed to send message to subscriber".to_string())
        })
    }

    /// Check if the subscriber is closed
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// Update the subscription configuration
    pub fn update_config(&mut self, config: SubscriptionConfig) {
        self.config = config;
    }
}

/// A subscriber for consuming messages
#[derive(Debug)]
pub struct Subscriber {
    /// Subscriber handle
    handle: SubscriberHandle,
    /// Message receiver
    receiver: MessageReceiver,
}

impl Subscriber {
    /// Create a new subscriber
    pub fn new(config: SubscriptionConfig) -> Self {
        let (handle, receiver) = SubscriberHandle::new(config);
        Self { handle, receiver }
    }

    /// Get the subscriber ID
    pub fn id(&self) -> &SubscriberId {
        self.handle.id()
    }

    /// Get the subscription configuration
    pub fn config(&self) -> &SubscriptionConfig {
        self.handle.config()
    }

    /// Get the topic pattern
    pub fn topic_pattern(&self) -> &str {
        self.handle.topic_pattern()
    }

    /// Receive the next message
    pub async fn next(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }

    /// Try to receive a message without blocking
    pub fn try_next(&mut self) -> Result<Option<Message>> {
        match self.receiver.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(PubSubError::subscription(
                "Subscriber disconnected".to_string(),
            )),
        }
    }

    /// Close the subscriber
    pub fn close(&mut self) {
        self.receiver.close();
    }

    /// Check if the subscriber is closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// Get the subscriber handle
    pub fn handle(&self) -> &SubscriberHandle {
        &self.handle
    }

    /// Split into handle and receiver
    pub fn into_parts(self) -> (SubscriberHandle, MessageReceiver) {
        (self.handle, self.receiver)
    }
}

/// Subscription manager for handling subscriber lifecycle
#[derive(Debug)]
pub struct SubscriptionManager {
    /// Active subscriptions
    subscriptions: Arc<RwLock<HashMap<SubscriberId, SubscriberHandle>>>,
    /// Pattern to subscriber mapping
    pattern_subscribers: Arc<RwLock<HashMap<String, Vec<SubscriberId>>>>,
}

impl SubscriptionManager {
    /// Create a new subscription manager
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            pattern_subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a subscription
    pub async fn subscribe(&self, pattern: &TopicPattern) -> Result<Subscriber> {
        let config = SubscriptionConfig::new(pattern.pattern());
        let subscriber = Subscriber::new(config.clone());

        // Store the subscription
        let mut subscriptions = self.subscriptions.write().await;
        let mut pattern_subscribers = self.pattern_subscribers.write().await;

        subscriptions.insert(subscriber.id().clone(), subscriber.handle().clone());

        pattern_subscribers
            .entry(pattern.pattern().to_string())
            .or_insert_with(Vec::new)
            .push(subscriber.id().clone());

        info!(
            "Created subscription for pattern '{}' with ID {}",
            pattern,
            subscriber.id()
        );
        Ok(subscriber)
    }

    /// Create a subscription with custom configuration
    pub async fn subscribe_with_config(&self, config: SubscriptionConfig) -> Result<Subscriber> {
        let subscriber = Subscriber::new(config.clone());

        // Store the subscription
        let mut subscriptions = self.subscriptions.write().await;
        let mut pattern_subscribers = self.pattern_subscribers.write().await;

        subscriptions.insert(subscriber.id().clone(), subscriber.handle().clone());

        pattern_subscribers
            .entry(config.topic_pattern.clone())
            .or_insert_with(Vec::new)
            .push(subscriber.id().clone());

        info!(
            "Created subscription for pattern '{}' with ID {}",
            config.topic_pattern,
            subscriber.id()
        );
        Ok(subscriber)
    }

    /// Remove a subscription
    pub async fn unsubscribe(&self, subscriber_id: &SubscriberId) -> Result<bool> {
        let mut subscriptions = self.subscriptions.write().await;
        let mut pattern_subscribers = self.pattern_subscribers.write().await;

        if let Some(handle) = subscriptions.remove(subscriber_id) {
            // Remove from pattern mapping
            if let Some(subscribers) = pattern_subscribers.get_mut(&handle.config.topic_pattern) {
                subscribers.retain(|id| id != subscriber_id);
                if subscribers.is_empty() {
                    pattern_subscribers.remove(&handle.config.topic_pattern);
                }
            }

            info!("Removed subscription with ID {}", subscriber_id);
            Ok(true)
        } else {
            warn!(
                "Attempted to remove non-existent subscription with ID {}",
                subscriber_id
            );
            Ok(false)
        }
    }

    /// Get a subscription by ID
    pub async fn get_subscription(
        &self,
        subscriber_id: &SubscriberId,
    ) -> Result<Option<SubscriberHandle>> {
        let subscriptions = self.subscriptions.read().await;
        Ok(subscriptions.get(subscriber_id).cloned())
    }

    /// Get all subscribers for a topic pattern
    pub async fn get_subscribers_for_pattern(
        &self,
        pattern: &str,
    ) -> Result<Vec<SubscriberHandle>> {
        let subscriptions = self.subscriptions.read().await;
        let pattern_subscribers = self.pattern_subscribers.read().await;

        let mut matching_subscribers = Vec::new();

        if let Some(subscriber_ids) = pattern_subscribers.get(pattern) {
            for subscriber_id in subscriber_ids {
                if let Some(handle) = subscriptions.get(subscriber_id) {
                    matching_subscribers.push(handle.clone());
                }
            }
        }

        debug!(
            "Found {} subscribers for pattern '{}'",
            matching_subscribers.len(),
            pattern
        );
        Ok(matching_subscribers)
    }

    /// Get all subscribers that match a topic
    pub async fn get_subscribers_for_topic(&self, topic: &str) -> Result<Vec<SubscriberHandle>> {
        let subscriptions = self.subscriptions.read().await;
        let pattern_subscribers = self.pattern_subscribers.read().await;

        let mut matching_subscribers = Vec::new();

        for (pattern_str, subscriber_ids) in pattern_subscribers.iter() {
            let pattern = TopicPattern::new(pattern_str);
            if pattern.matches(topic) {
                for subscriber_id in subscriber_ids {
                    if let Some(handle) = subscriptions.get(subscriber_id) {
                        matching_subscribers.push(handle.clone());
                    }
                }
            }
        }

        debug!(
            "Found {} subscribers for topic '{}'",
            matching_subscribers.len(),
            topic
        );
        Ok(matching_subscribers)
    }

    /// List all active subscriptions
    pub async fn list_subscriptions(&self) -> Result<Vec<SubscriberHandle>> {
        let subscriptions = self.subscriptions.read().await;
        Ok(subscriptions.values().cloned().collect())
    }

    /// List all subscription patterns
    pub async fn list_patterns(&self) -> Result<Vec<String>> {
        let pattern_subscribers = self.pattern_subscribers.read().await;
        Ok(pattern_subscribers.keys().cloned().collect())
    }

    /// Get the number of active subscriptions
    pub async fn subscription_count(&self) -> Result<usize> {
        let subscriptions = self.subscriptions.read().await;
        Ok(subscriptions.len())
    }

    /// Get the number of subscribers for a pattern
    pub async fn pattern_subscriber_count(&self, pattern: &str) -> Result<usize> {
        let pattern_subscribers = self.pattern_subscribers.read().await;
        Ok(pattern_subscribers
            .get(pattern)
            .map(|v| v.len())
            .unwrap_or(0))
    }

    /// Clear all subscriptions (use with caution)
    pub async fn clear_all_subscriptions(&self) -> Result<usize> {
        let mut subscriptions = self.subscriptions.write().await;
        let mut pattern_subscribers = self.pattern_subscribers.write().await;

        let count = subscriptions.len();
        subscriptions.clear();
        pattern_subscribers.clear();

        warn!("Cleared all {} subscriptions", count);
        Ok(count)
    }

    /// Deliver a message to all matching subscribers
    pub async fn deliver_message(&self, topic: &str, message: Message) -> Result<usize> {
        let subscribers = self.get_subscribers_for_topic(topic).await?;
        let mut delivered_count = 0;

        for subscriber in subscribers {
            if !subscriber.is_closed() {
                if let Err(e) = subscriber.send_message(message.clone()).await {
                    warn!(
                        "Failed to deliver message to subscriber {}: {}",
                        subscriber.id(),
                        e
                    );
                } else {
                    delivered_count += 1;
                }
            }
        }

        debug!(
            "Delivered message to {} subscribers for topic '{}'",
            delivered_count, topic
        );
        Ok(delivered_count)
    }

    /// Clean up closed subscriptions
    pub async fn cleanup_closed_subscriptions(&self) -> Result<usize> {
        let mut subscriptions = self.subscriptions.write().await;
        let mut pattern_subscribers = self.pattern_subscribers.write().await;

        let mut closed_subscribers = Vec::new();

        // Find closed subscriptions
        for (id, handle) in subscriptions.iter() {
            if handle.is_closed() {
                closed_subscribers.push((id.clone(), handle.config.topic_pattern.clone()));
            }
        }

        // Remove closed subscriptions
        for (id, pattern) in &closed_subscribers {
            subscriptions.remove(id);

            if let Some(subscribers) = pattern_subscribers.get_mut(pattern) {
                subscribers.retain(|subscriber_id| subscriber_id != id);
                if subscribers.is_empty() {
                    pattern_subscribers.remove(pattern);
                }
            }
        }

        let count = closed_subscribers.len();
        if count > 0 {
            info!("Cleaned up {} closed subscriptions", count);
        }

        Ok(count)
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SubscriberHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            sender: self.sender.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_subscriber_handle_creation() {
        let config = SubscriptionConfig::new("test.*");
        let (handle, _receiver) = SubscriberHandle::new(config.clone());

        assert_eq!(handle.id(), &config.subscriber_id);
        assert_eq!(handle.topic_pattern(), "test.*");
        assert!(!handle.is_closed());
    }

    #[tokio::test]
    async fn test_subscriber_handle_send_message() {
        let config = SubscriptionConfig::new("test.*");
        let (handle, mut receiver) = SubscriberHandle::new(config);
        let message = Message::new(json!({"test": "data"}));

        let result = handle.send_message(message.clone()).await;
        assert!(result.is_ok());

        let received = receiver.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload(), message.payload());
    }

    #[test]
    fn test_subscriber_creation() {
        let config = SubscriptionConfig::new("test.*");
        let subscriber = Subscriber::new(config.clone());

        assert_eq!(subscriber.id(), &config.subscriber_id);
        assert_eq!(subscriber.topic_pattern(), "test.*");
        assert!(!subscriber.is_closed());
    }

    #[tokio::test]
    async fn test_subscriber_message_receiving() {
        let config = SubscriptionConfig::new("test.*");
        let mut subscriber = Subscriber::new(config);
        let message = Message::new(json!({"test": "data"}));

        // Send message through handle
        let result = subscriber.handle().send_message(message.clone()).await;
        assert!(result.is_ok());

        // Receive message
        let received = subscriber.next().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload(), message.payload());
    }

    #[tokio::test]
    async fn test_subscriber_try_next() {
        let config = SubscriptionConfig::new("test.*");
        let mut subscriber = Subscriber::new(config);

        // Should return None when no messages
        let result = subscriber.try_next().unwrap();
        assert!(result.is_none());

        // Send a message
        let message = Message::new(json!({"test": "data"}));
        subscriber
            .handle()
            .send_message(message.clone())
            .await
            .unwrap();

        // Should receive the message
        let result = subscriber.try_next().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().payload(), message.payload());
    }

    #[tokio::test]
    async fn test_subscription_manager_subscribe() {
        let manager = SubscriptionManager::new();
        let pattern = TopicPattern::new("test.*");

        let subscriber = manager.subscribe(&pattern).await.unwrap();
        assert_eq!(subscriber.topic_pattern(), "test.*");

        let count = manager.subscription_count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_subscription_manager_unsubscribe() {
        let manager = SubscriptionManager::new();
        let pattern = TopicPattern::new("test.*");

        let subscriber = manager.subscribe(&pattern).await.unwrap();
        let subscriber_id = subscriber.id().clone();

        let result = manager.unsubscribe(&subscriber_id).await.unwrap();
        assert!(result);

        let count = manager.subscription_count().await.unwrap();
        assert_eq!(count, 0);

        // Should return false for non-existent subscription
        let result = manager.unsubscribe(&subscriber_id).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_subscription_manager_get_subscribers_for_topic() {
        let manager = SubscriptionManager::new();

        // Create subscriptions with different patterns
        manager
            .subscribe(&TopicPattern::new("test.*"))
            .await
            .unwrap();
        manager
            .subscribe(&TopicPattern::new("test.function.*"))
            .await
            .unwrap();
        manager
            .subscribe(&TopicPattern::new("other.*"))
            .await
            .unwrap();

        // Test topic matching
        let subscribers = manager
            .get_subscribers_for_topic("test.function.completed")
            .await
            .unwrap();
        assert_eq!(subscribers.len(), 2); // Should match "test.*" and "test.function.*"

        let subscribers = manager
            .get_subscribers_for_topic("other.event")
            .await
            .unwrap();
        assert_eq!(subscribers.len(), 1); // Should match "other.*"

        let subscribers = manager
            .get_subscribers_for_topic("unmatched.topic")
            .await
            .unwrap();
        assert_eq!(subscribers.len(), 0); // Should match nothing
    }

    #[tokio::test]
    async fn test_subscription_manager_deliver_message() {
        let manager = SubscriptionManager::new();

        // Create subscriptions
        let mut subscriber1 = manager
            .subscribe(&TopicPattern::new("test.*"))
            .await
            .unwrap();
        let mut subscriber2 = manager
            .subscribe(&TopicPattern::new("test.function.*"))
            .await
            .unwrap();

        // Deliver message
        let message = Message::new_with_topic("test.function.completed", json!({"test": "data"}));
        let delivered_count = manager
            .deliver_message("test.function.completed", message.clone())
            .await
            .unwrap();
        assert_eq!(delivered_count, 2);

        // Check that both subscribers received the message
        let received1 = subscriber1.next().await;
        assert!(received1.is_some());
        assert_eq!(received1.unwrap().payload(), message.payload());

        let received2 = subscriber2.next().await;
        assert!(received2.is_some());
        assert_eq!(received2.unwrap().payload(), message.payload());
    }

    #[tokio::test]
    async fn test_subscription_manager_cleanup_closed_subscriptions() {
        let manager = SubscriptionManager::new();

        // Create subscriptions
        let mut subscriber1 = manager
            .subscribe(&TopicPattern::new("test.*"))
            .await
            .unwrap();
        let _subscriber2 = manager
            .subscribe(&TopicPattern::new("other.*"))
            .await
            .unwrap();

        assert_eq!(manager.subscription_count().await.unwrap(), 2);

        // Close one subscriber
        subscriber1.close();

        // Cleanup should remove the closed subscription
        let cleaned_count = manager.cleanup_closed_subscriptions().await.unwrap();
        assert_eq!(cleaned_count, 1);
        assert_eq!(manager.subscription_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_subscription_manager_list_operations() {
        let manager = SubscriptionManager::new();

        // Create subscriptions
        manager
            .subscribe(&TopicPattern::new("test.*"))
            .await
            .unwrap();
        manager
            .subscribe(&TopicPattern::new("other.*"))
            .await
            .unwrap();

        let subscriptions = manager.list_subscriptions().await.unwrap();
        assert_eq!(subscriptions.len(), 2);

        let patterns = manager.list_patterns().await.unwrap();
        assert_eq!(patterns.len(), 2);
        assert!(patterns.contains(&"test.*".to_string()));
        assert!(patterns.contains(&"other.*".to_string()));
    }

    #[tokio::test]
    async fn test_subscription_manager_clear_all() {
        let manager = SubscriptionManager::new();

        // Create subscriptions
        manager
            .subscribe(&TopicPattern::new("test.*"))
            .await
            .unwrap();
        manager
            .subscribe(&TopicPattern::new("other.*"))
            .await
            .unwrap();

        let cleared_count = manager.clear_all_subscriptions().await.unwrap();
        assert_eq!(cleared_count, 2);
        assert_eq!(manager.subscription_count().await.unwrap(), 0);
    }
}
