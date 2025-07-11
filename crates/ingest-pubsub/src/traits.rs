use crate::{Message, Result, Topic, TopicPattern};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Publisher trait for publishing messages
#[async_trait]
pub trait Publisher: Send + Sync {
    /// Publish a message to a topic
    async fn publish(&self, topic: &Topic, message: Message) -> Result<()>;

    /// Publish multiple messages to a topic
    async fn publish_batch(&self, topic: &Topic, messages: Vec<Message>) -> Result<()> {
        for message in messages {
            self.publish(topic, message).await?;
        }
        Ok(())
    }

    /// Publish a message and wait for acknowledgment
    async fn publish_and_wait(&self, topic: &Topic, message: Message) -> Result<()> {
        // Default implementation just publishes
        self.publish(topic, message).await
    }
}

/// Subscriber trait for consuming messages
#[async_trait]
pub trait SubscriberTrait: Send + Sync {
    /// Subscribe to a topic pattern
    async fn subscribe(&self, pattern: &TopicPattern) -> Result<MessageReceiver>;

    /// Unsubscribe from a topic pattern
    async fn unsubscribe(&self, pattern: &TopicPattern) -> Result<()>;

    /// Get the list of subscribed patterns
    async fn subscriptions(&self) -> Result<Vec<TopicPattern>>;

    /// Check if subscribed to a pattern
    async fn is_subscribed(&self, pattern: &TopicPattern) -> Result<bool> {
        let subscriptions = self.subscriptions().await?;
        Ok(subscriptions.contains(pattern))
    }
}

/// Message receiver for consuming messages
pub type MessageReceiver = mpsc::UnboundedReceiver<Message>;

/// Message sender for publishing messages
pub type MessageSender = mpsc::UnboundedSender<Message>;

/// Acknowledgment trait for message acknowledgment
#[async_trait]
pub trait Acknowledger: Send + Sync {
    /// Acknowledge successful message processing
    async fn ack(&self, message: &Message) -> Result<()>;

    /// Negative acknowledgment (reject and potentially requeue)
    async fn nack(&self, message: &Message, requeue: bool) -> Result<()>;

    /// Acknowledge multiple messages
    async fn ack_batch(&self, messages: &[Message]) -> Result<()> {
        for message in messages {
            self.ack(message).await?;
        }
        Ok(())
    }

    /// Negative acknowledgment for multiple messages
    async fn nack_batch(&self, messages: &[Message], requeue: bool) -> Result<()> {
        for message in messages {
            self.nack(message, requeue).await?;
        }
        Ok(())
    }
}

/// Message handler trait for processing messages
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle a received message
    async fn handle_message(&self, message: Message) -> Result<()>;

    /// Get the handler name for identification
    fn name(&self) -> &str;

    /// Check if this handler can process the given message
    fn can_handle(&self, _message: &Message) -> bool {
        // Default implementation accepts all messages
        true
    }

    /// Handle message processing errors
    async fn handle_error(
        &self,
        message: &Message,
        error: &(dyn std::error::Error + Send + Sync),
    ) -> Result<()> {
        // Default implementation logs the error
        tracing::error!(
            "Message handler '{}' failed to process message {}: {}",
            self.name(),
            message.id(),
            error
        );
        Ok(())
    }
}

/// Topic lifecycle listener trait
#[async_trait]
pub trait TopicLifecycleListener: Send + Sync {
    /// Called when a topic is created
    async fn on_topic_created(&self, topic: &Topic) -> Result<()>;

    /// Called when a topic is updated
    async fn on_topic_updated(&self, topic: &Topic) -> Result<()>;

    /// Called when a topic is deleted
    async fn on_topic_deleted(&self, topic_name: &str) -> Result<()>;
}

/// Subscription lifecycle listener trait
#[async_trait]
pub trait SubscriptionLifecycleListener: Send + Sync {
    /// Called when a subscription is created
    async fn on_subscription_created(&self, pattern: &TopicPattern) -> Result<()>;

    /// Called when a subscription is updated
    async fn on_subscription_updated(&self, pattern: &TopicPattern) -> Result<()>;

    /// Called when a subscription is deleted
    async fn on_subscription_deleted(&self, pattern: &TopicPattern) -> Result<()>;
}

/// Message filter trait for filtering messages
pub trait MessageFilter: Send + Sync {
    /// Check if a message should be delivered to a subscriber
    fn should_deliver(&self, message: &Message, pattern: &TopicPattern) -> bool;

    /// Get the filter name for identification
    fn name(&self) -> &str;
}

/// Message transformer trait for transforming messages
#[async_trait]
pub trait MessageTransformer: Send + Sync {
    /// Transform a message before delivery
    async fn transform(&self, message: Message) -> Result<Message>;

    /// Get the transformer name for identification
    fn name(&self) -> &str;

    /// Check if this transformer should process the given message
    fn should_transform(&self, _message: &Message) -> bool {
        // Default implementation transforms all messages
        true
    }
}

/// Dead letter handler trait for handling failed messages
#[async_trait]
pub trait DeadLetterHandler: Send + Sync {
    /// Handle a message that has exceeded retry limits
    async fn handle_dead_letter(&self, message: Message, reason: &str) -> Result<()>;

    /// Get the handler name for identification
    fn name(&self) -> &str;
}

/// Metrics collector trait for collecting pub-sub metrics
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a message published
    async fn record_message_published(&self, topic: &str, size: usize) -> Result<()>;

    /// Record a message delivered
    async fn record_message_delivered(&self, topic: &str, subscriber: &str) -> Result<()>;

    /// Record a message acknowledged
    async fn record_message_acknowledged(&self, topic: &str, subscriber: &str) -> Result<()>;

    /// Record a message rejected
    async fn record_message_rejected(
        &self,
        topic: &str,
        subscriber: &str,
        reason: &str,
    ) -> Result<()>;

    /// Record a subscription created
    async fn record_subscription_created(&self, pattern: &str) -> Result<()>;

    /// Record a subscription deleted
    async fn record_subscription_deleted(&self, pattern: &str) -> Result<()>;

    /// Record a topic created
    async fn record_topic_created(&self, topic: &str) -> Result<()>;

    /// Record a topic deleted
    async fn record_topic_deleted(&self, topic: &str) -> Result<()>;
}

/// Health check trait for pub-sub components
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Check if the component is healthy
    async fn health_check(&self) -> Result<HealthStatus>;

    /// Get the component name for identification
    fn name(&self) -> &str;
}

/// Health status for components
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component is degraded but functional
    Degraded { reason: String },
    /// Component is unhealthy
    Unhealthy { reason: String },
}

impl HealthStatus {
    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Check if the status is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded { .. })
    }

    /// Check if the status is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy { .. })
    }

    /// Get the reason for degraded/unhealthy status
    pub fn reason(&self) -> Option<&str> {
        match self {
            HealthStatus::Healthy => None,
            HealthStatus::Degraded { reason } => Some(reason),
            HealthStatus::Unhealthy { reason } => Some(reason),
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded { reason } => write!(f, "Degraded: {reason}"),
            HealthStatus::Unhealthy { reason } => write!(f, "Unhealthy: {reason}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    struct TestMessageHandler {
        name: String,
    }

    impl TestMessageHandler {
        fn new(name: impl Into<String>) -> Self {
            Self { name: name.into() }
        }
    }

    #[async_trait]
    impl MessageHandler for TestMessageHandler {
        async fn handle_message(&self, _message: Message) -> Result<()> {
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct TestMessageFilter {
        name: String,
    }

    impl TestMessageFilter {
        fn new(name: impl Into<String>) -> Self {
            Self { name: name.into() }
        }
    }

    impl MessageFilter for TestMessageFilter {
        fn should_deliver(&self, _message: &Message, _pattern: &TopicPattern) -> bool {
            true
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_health_status_creation() {
        let healthy = HealthStatus::Healthy;
        assert!(healthy.is_healthy());
        assert!(!healthy.is_degraded());
        assert!(!healthy.is_unhealthy());
        assert_eq!(healthy.reason(), None);

        let degraded = HealthStatus::Degraded {
            reason: "High latency".to_string(),
        };
        assert!(!degraded.is_healthy());
        assert!(degraded.is_degraded());
        assert!(!degraded.is_unhealthy());
        assert_eq!(degraded.reason(), Some("High latency"));

        let unhealthy = HealthStatus::Unhealthy {
            reason: "Connection lost".to_string(),
        };
        assert!(!unhealthy.is_healthy());
        assert!(!unhealthy.is_degraded());
        assert!(unhealthy.is_unhealthy());
        assert_eq!(unhealthy.reason(), Some("Connection lost"));
    }

    #[test]
    fn test_health_status_display() {
        let healthy = HealthStatus::Healthy;
        assert_eq!(format!("{}", healthy), "Healthy");

        let degraded = HealthStatus::Degraded {
            reason: "High latency".to_string(),
        };
        assert_eq!(format!("{}", degraded), "Degraded: High latency");

        let unhealthy = HealthStatus::Unhealthy {
            reason: "Connection lost".to_string(),
        };
        assert_eq!(format!("{}", unhealthy), "Unhealthy: Connection lost");
    }

    #[tokio::test]
    async fn test_message_handler_trait() {
        let fixture = TestMessageHandler::new("test-handler");
        let message = Message::new(json!({"test": "data"}));

        assert_eq!(fixture.name(), "test-handler");
        assert!(fixture.can_handle(&message));

        let result = fixture.handle_message(message).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_filter_trait() {
        let fixture = TestMessageFilter::new("test-filter");
        let message = Message::new(json!({"test": "data"}));
        let pattern = TopicPattern::new("test.*");

        assert_eq!(fixture.name(), "test-filter");
        assert!(fixture.should_deliver(&message, &pattern));
    }

    #[tokio::test]
    async fn test_message_handler_error_handling() {
        let fixture = TestMessageHandler::new("test-handler");
        let message = Message::new(json!({"test": "data"}));
        let error = std::io::Error::other("test error");

        let result = fixture.handle_error(&message, &error).await;
        assert!(result.is_ok());
    }
}
