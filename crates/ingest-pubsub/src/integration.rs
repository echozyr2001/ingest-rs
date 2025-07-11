use crate::{ManagerStats, Message, PubSubManager, Result, Topic, TopicPattern};
use ingest_core::Id;
use ingest_execution::{ExecutionRequest, ExecutionResult};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

/// Integration layer for connecting PubSub with execution engine
pub struct ExecutionIntegration {
    /// PubSub manager for publishing events
    pubsub: Arc<PubSubManager>,
    /// Configuration for event publishing
    config: IntegrationConfig,
}

/// Configuration for execution integration
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    /// Whether to publish execution start events
    pub publish_start_events: bool,
    /// Whether to publish execution completion events
    pub publish_completion_events: bool,
    /// Whether to publish execution failure events
    pub publish_failure_events: bool,
    /// Whether to publish step execution events
    pub publish_step_events: bool,
    /// Custom topic prefix for execution events
    pub topic_prefix: String,
    /// Additional metadata to include in all events
    pub default_metadata: HashMap<String, String>,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            publish_start_events: true,
            publish_completion_events: true,
            publish_failure_events: true,
            publish_step_events: false,
            topic_prefix: "execution".to_string(),
            default_metadata: HashMap::new(),
        }
    }
}

impl ExecutionIntegration {
    /// Create a new execution integration
    pub fn new(pubsub: Arc<PubSubManager>) -> Self {
        Self {
            pubsub,
            config: IntegrationConfig::default(),
        }
    }

    /// Create integration with custom configuration
    pub fn with_config(pubsub: Arc<PubSubManager>, config: IntegrationConfig) -> Self {
        Self { pubsub, config }
    }

    /// Publish execution start event
    pub async fn publish_execution_start(&self, request: &ExecutionRequest) -> Result<()> {
        if !self.config.publish_start_events {
            return Ok(());
        }

        let topic = Topic::new(format!("{}.started", self.config.topic_prefix));
        let mut message = Message::new(json!({
            "run_id": request.run_id,
            "function_id": request.function.id,
            "function_name": request.function.name,
            "event_name": request.event.name,
            "event_id": request.event.id,
            "started_at": chrono::Utc::now()
        }));

        // Add default metadata as headers
        for (key, value) in &self.config.default_metadata {
            message.add_header(key, value);
        }
        message.add_header("event_type", "execution.started");
        message.add_header("source", "execution_integration");

        self.pubsub.publish(&topic, message).await
    }

    /// Publish execution completion event
    pub async fn publish_execution_completion(&self, result: &ExecutionResult) -> Result<()> {
        if !self.config.publish_completion_events {
            return Ok(());
        }

        let topic = Topic::new(format!("{}.completed", self.config.topic_prefix));
        let mut message = Message::new(json!({
            "run_id": result.run_id,
            "function_id": result.function_id,
            "status": result.status.to_string(),
            "output": result.output,
            "duration_ms": result.duration.map(|d| d.as_millis()),
            "completed_at": result.completed_at
        }));

        // Add default metadata as headers
        for (key, value) in &self.config.default_metadata {
            message.add_header(key, value);
        }
        message.add_header("event_type", "execution.completed");
        message.add_header("source", "execution_integration");

        self.pubsub.publish(&topic, message).await
    }

    /// Publish execution failure event
    pub async fn publish_execution_failure(&self, result: &ExecutionResult) -> Result<()> {
        if !self.config.publish_failure_events {
            return Ok(());
        }

        let topic = Topic::new(format!("{}.failed", self.config.topic_prefix));
        let mut message = Message::new(json!({
            "run_id": result.run_id,
            "function_id": result.function_id,
            "error": result.error,
            "duration_ms": result.duration.map(|d| d.as_millis()),
            "failed_at": result.completed_at
        }));

        // Add default metadata as headers
        for (key, value) in &self.config.default_metadata {
            message.add_header(key, value);
        }
        message.add_header("event_type", "execution.failed");
        message.add_header("source", "execution_integration");

        self.pubsub.publish(&topic, message).await
    }

    /// Publish step execution event
    pub async fn publish_step_execution(
        &self,
        run_id: &Id,
        function_id: &Id,
        step_name: &str,
        step_output: &serde_json::Value,
    ) -> Result<()> {
        if !self.config.publish_step_events {
            return Ok(());
        }

        let topic = Topic::new(format!("{}.step", self.config.topic_prefix));
        let mut message = Message::new(json!({
            "run_id": run_id,
            "function_id": function_id,
            "step_name": step_name,
            "step_output": step_output,
            "executed_at": chrono::Utc::now()
        }));

        // Add default metadata as headers
        for (key, value) in &self.config.default_metadata {
            message.add_header(key, value);
        }
        message.add_header("event_type", "execution.step");
        message.add_header("source", "execution_integration");

        self.pubsub.publish(&topic, message).await
    }

    /// Subscribe to execution events
    pub async fn subscribe_to_execution_events(&self) -> Result<crate::Subscriber> {
        let pattern = TopicPattern::new(format!("{}.*", self.config.topic_prefix));
        self.pubsub.subscribe(&pattern).await
    }

    /// Subscribe to specific execution event type
    pub async fn subscribe_to_event_type(&self, event_type: &str) -> Result<crate::Subscriber> {
        let pattern = TopicPattern::new(format!("{}.{}", self.config.topic_prefix, event_type));
        self.pubsub.subscribe(&pattern).await
    }

    /// Get integration statistics
    pub async fn get_stats(&self) -> IntegrationStats {
        let manager_stats = self.pubsub.get_manager_stats().await.unwrap_or({
            // Return default stats if error
            ManagerStats {
                running: false,
                total_messages_processed: 0,
                total_subscriptions_created: 0,
                current_topic_count: 0,
                current_subscription_count: 0,
                messages_published: 0,
                messages_delivered: 0,
                failed_deliveries: 0,
                delivery_success_rate: 0.0,
            }
        });
        IntegrationStats {
            total_published: manager_stats.total_messages_processed,
            total_subscriptions: manager_stats.total_subscriptions_created,
            active_topics: manager_stats.current_topic_count,
        }
    }
}

/// Statistics for execution integration
#[derive(Debug, Clone)]
pub struct IntegrationStats {
    /// Total messages published
    pub total_published: u64,
    /// Total subscriptions created
    pub total_subscriptions: u64,
    /// Number of active topics
    pub active_topics: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::{Event, Function, FunctionTrigger};
    use pretty_assertions::assert_eq;
    use serde_json::json;

    async fn create_test_pubsub() -> Arc<PubSubManager> {
        let manager = PubSubManager::new().await.unwrap();
        manager.start().await.unwrap();
        Arc::new(manager)
    }

    fn create_test_function() -> Function {
        Function::new("test-function").add_trigger(FunctionTrigger::event("user.created"))
    }

    fn create_test_event() -> Event {
        Event::new("user.created", json!({"user_id": 123}))
    }

    fn create_test_request() -> ExecutionRequest {
        ExecutionRequest::new(create_test_function(), create_test_event())
    }

    #[tokio::test]
    async fn test_integration_creation() {
        let fixture_pubsub = create_test_pubsub().await;
        let actual = ExecutionIntegration::new(fixture_pubsub);

        assert_eq!(actual.config.topic_prefix, "execution");
        assert!(actual.config.publish_start_events);
        assert!(actual.config.publish_completion_events);
        assert!(actual.config.publish_failure_events);
        assert!(!actual.config.publish_step_events);
    }

    #[tokio::test]
    async fn test_integration_with_config() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_config = IntegrationConfig {
            topic_prefix: "custom".to_string(),
            publish_step_events: true,
            ..IntegrationConfig::default()
        };

        let actual = ExecutionIntegration::with_config(fixture_pubsub, fixture_config.clone());
        assert_eq!(actual.config.topic_prefix, "custom");
        assert!(actual.config.publish_step_events);
    }

    #[tokio::test]
    async fn test_publish_execution_start() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);
        let fixture_request = create_test_request();

        let actual = fixture_integration
            .publish_execution_start(&fixture_request)
            .await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_publish_execution_start_disabled() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_config = IntegrationConfig {
            publish_start_events: false,
            ..IntegrationConfig::default()
        };
        let fixture_integration = ExecutionIntegration::with_config(fixture_pubsub, fixture_config);
        let fixture_request = create_test_request();

        let actual = fixture_integration
            .publish_execution_start(&fixture_request)
            .await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_publish_execution_completion() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);
        let fixture_result = ExecutionResult::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );

        let actual = fixture_integration
            .publish_execution_completion(&fixture_result)
            .await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_publish_execution_failure() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);
        let mut fixture_result = ExecutionResult::new(
            ingest_core::generate_id_with_prefix("run"),
            ingest_core::generate_id_with_prefix("fn"),
        );
        fixture_result.fail("Test error");

        let actual = fixture_integration
            .publish_execution_failure(&fixture_result)
            .await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_publish_step_execution() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_config = IntegrationConfig {
            publish_step_events: true,
            ..IntegrationConfig::default()
        };
        let fixture_integration = ExecutionIntegration::with_config(fixture_pubsub, fixture_config);

        let fixture_run_id = ingest_core::generate_id_with_prefix("run");
        let fixture_function_id = ingest_core::generate_id_with_prefix("fn");
        let fixture_output = json!({"result": "success"});

        let actual = fixture_integration
            .publish_step_execution(
                &fixture_run_id,
                &fixture_function_id,
                "test-step",
                &fixture_output,
            )
            .await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_to_execution_events() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);

        let actual = fixture_integration.subscribe_to_execution_events().await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_to_event_type() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);

        let actual = fixture_integration.subscribe_to_event_type("started").await;
        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_get_stats() {
        let fixture_pubsub = create_test_pubsub().await;
        let fixture_integration = ExecutionIntegration::new(fixture_pubsub);

        let actual = fixture_integration.get_stats().await;
        assert_eq!(actual.total_published, 0);
        assert_eq!(actual.total_subscriptions, 0);
        assert_eq!(actual.active_topics, 0);
    }

    #[tokio::test]
    async fn test_integration_config_default() {
        let actual = IntegrationConfig::default();
        let expected = IntegrationConfig {
            publish_start_events: true,
            publish_completion_events: true,
            publish_failure_events: true,
            publish_step_events: false,
            topic_prefix: "execution".to_string(),
            default_metadata: HashMap::new(),
        };
        assert_eq!(actual.publish_start_events, expected.publish_start_events);
        assert_eq!(
            actual.publish_completion_events,
            expected.publish_completion_events
        );
        assert_eq!(
            actual.publish_failure_events,
            expected.publish_failure_events
        );
        assert_eq!(actual.publish_step_events, expected.publish_step_events);
        assert_eq!(actual.topic_prefix, expected.topic_prefix);
        assert_eq!(actual.default_metadata, expected.default_metadata);
    }
}
