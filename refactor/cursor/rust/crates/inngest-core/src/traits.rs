//! Core traits for Inngest components
//!
//! This module defines the fundamental traits that establish contracts
//! between different parts of the Inngest system.

use crate::error::Result;
use crate::event::Event;
use crate::{ExecutionContext, ExecutionResult, StateIdentifier};
use async_trait::async_trait;
use std::collections::HashMap;
use ulid::Ulid;
use uuid::Uuid;

/// Trait for handling events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle a single event
    async fn handle_event(&self, event: Event) -> Result<()>;

    /// Handle a batch of events
    async fn handle_events(&self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.handle_event(event).await?;
        }
        Ok(())
    }
}

/// Trait for executing functions
#[async_trait]
pub trait FunctionExecutor: Send + Sync {
    /// Execute a function with the given context
    async fn execute(&self, context: ExecutionContext) -> Result<ExecutionResult>;

    /// Schedule a function for execution
    async fn schedule(&self, request: ScheduleRequest) -> Result<RunMetadata>;
}

/// Trait for managing function execution state
#[async_trait]
pub trait StateManager: Send + Sync {
    /// Create new state for a function run
    async fn create_state(&self, input: StateInput) -> Result<ExecutionState>;

    /// Load existing state by identifier
    async fn load_state(&self, id: StateIdentifier) -> Result<ExecutionState>;

    /// Save a step result to state
    async fn save_step(&self, id: StateIdentifier, step_id: String, data: Vec<u8>) -> Result<bool>;

    /// Delete state (cleanup after function completion)
    async fn delete_state(&self, id: StateIdentifier) -> Result<bool>;

    /// Check if state exists
    async fn state_exists(&self, id: StateIdentifier) -> Result<bool>;

    /// Update metadata for a function run
    async fn update_metadata(&self, id: StateIdentifier, metadata: RunMetadata) -> Result<()>;
}

/// Trait for queue management
#[async_trait]
pub trait QueueManager: Send + Sync {
    /// Add an item to the queue
    async fn enqueue(
        &self,
        item: QueueItem,
        schedule_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<()>;

    /// Remove and return the next item from the queue
    async fn dequeue(&self, shard: QueueShard) -> Result<Option<QueueItem>>;

    /// Start the queue processor
    async fn start_processing<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(QueueItem) -> Result<QueueProcessResult> + Send + Sync + 'static;

    /// Get queue statistics
    async fn stats(&self) -> Result<QueueStats>;
}

/// Trait for pub/sub messaging
#[async_trait]
pub trait MessagePublisher: Send + Sync {
    /// Publish a message to a topic
    async fn publish(&self, topic: &str, message: &[u8]) -> Result<()>;
}

#[async_trait]
pub trait MessageSubscriber: Send + Sync {
    /// Subscribe to a topic with a message handler
    async fn subscribe<F>(&self, topic: &str, handler: F) -> Result<()>
    where
        F: Fn(&[u8]) -> Result<()> + Send + Sync + 'static;
}

/// Combined pub/sub trait
pub trait MessageBroker: MessagePublisher + MessageSubscriber + Send + Sync {}

/// Trait for configuration management
pub trait ConfigProvider: Send + Sync {
    /// Get a configuration value by key
    fn get_string(&self, key: &str) -> Option<String>;

    /// Get a boolean configuration value
    fn get_bool(&self, key: &str) -> Option<bool>;

    /// Get an integer configuration value  
    fn get_int(&self, key: &str) -> Option<i64>;

    /// Check if a key exists
    fn has_key(&self, key: &str) -> bool;

    /// Get all configuration as a map
    fn get_all(&self) -> HashMap<String, String>;
}

/// Trait for observability and metrics
pub trait MetricsRecorder: Send + Sync {
    /// Record a counter increment
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);

    /// Record a histogram value
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Record a gauge value
    fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}

// Supporting types for the traits

/// Request to schedule a function
#[derive(Debug, Clone)]
pub struct ScheduleRequest {
    pub workspace_id: Uuid,
    pub app_id: Uuid,
    pub function_id: Uuid,
    pub events: Vec<Event>,
    pub idempotency_key: Option<String>,
    pub account_id: Uuid,
    pub prevent_debounce: bool,
}

/// Metadata about a function run
#[derive(Debug, Clone)]
pub struct RunMetadata {
    pub run_id: Ulid,
    pub function_id: Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: RunStatus,
    pub config: RunConfig,
}

/// Status of a function run
#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Configuration for a function run
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub force_step_plan: bool,
    pub request_version: i32,
}

/// Input for creating new state
#[derive(Debug, Clone)]
pub struct StateInput {
    pub identifier: StateIdentifier,
    pub events: Vec<serde_json::Value>,
    pub steps: Vec<MemoizedStep>,
    pub step_inputs: Vec<MemoizedStep>,
}

/// Execution state for a function run
#[derive(Debug, Clone)]
pub struct ExecutionState {
    pub identifier: StateIdentifier,
    pub metadata: RunMetadata,
    pub events: Vec<serde_json::Value>,
    pub actions: HashMap<String, serde_json::Value>,
    pub stack: Vec<String>,
}

/// A memoized step result
#[derive(Debug, Clone)]
pub struct MemoizedStep {
    pub id: String,
    pub data: serde_json::Value,
}

/// Queue item for processing
#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: String,
    pub kind: QueueItemKind,
    pub payload: serde_json::Value,
    pub max_attempts: Option<u32>,
    pub attempt: u32,
    pub priority: Option<u32>,
    pub scheduled_for: chrono::DateTime<chrono::Utc>,
}

/// Type of queue item
#[derive(Debug, Clone, PartialEq)]
pub enum QueueItemKind {
    Start,
    Edge,
    Sleep,
    EdgeError,
    Pause,
    Debounce,
    Cancel,
}

/// Queue shard information
#[derive(Debug, Clone)]
pub struct QueueShard {
    pub name: String,
    pub kind: QueueShardKind,
    pub config: HashMap<String, String>,
}

/// Type of queue shard
#[derive(Debug, Clone, PartialEq)]
pub enum QueueShardKind {
    Redis,
    Memory,
}

/// Result of processing a queue item
#[derive(Debug, Clone)]
pub struct QueueProcessResult {
    pub success: bool,
    pub retry_after: Option<chrono::DateTime<chrono::Utc>>,
    pub output: Option<serde_json::Value>,
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub pending_items: u64,
    pub processing_items: u64,
    pub failed_items: u64,
    pub completed_items: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;

    // Mock implementations for testing
    struct MockEventHandler;

    #[async_trait]
    impl EventHandler for MockEventHandler {
        async fn handle_event(&self, _event: Event) -> Result<()> {
            Ok(())
        }
    }

    struct MockConfigProvider {
        values: HashMap<String, String>,
    }

    impl ConfigProvider for MockConfigProvider {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }

        fn get_bool(&self, key: &str) -> Option<bool> {
            self.get_string(key)?.parse().ok()
        }

        fn get_int(&self, key: &str) -> Option<i64> {
            self.get_string(key)?.parse().ok()
        }

        fn has_key(&self, key: &str) -> bool {
            self.values.contains_key(key)
        }

        fn get_all(&self) -> HashMap<String, String> {
            self.values.clone()
        }
    }

    #[tokio::test]
    async fn test_event_handler() {
        let handler = MockEventHandler;
        let event = Event::new("test", serde_json::json!({}));

        assert!(handler.handle_event(event).await.is_ok());
    }

    #[test]
    fn test_config_provider() {
        let mut values = HashMap::new();
        values.insert("key1".to_string(), "value1".to_string());
        values.insert("key2".to_string(), "true".to_string());
        values.insert("key3".to_string(), "42".to_string());

        let provider = MockConfigProvider { values };

        assert_eq!(provider.get_string("key1"), Some("value1".to_string()));
        assert_eq!(provider.get_bool("key2"), Some(true));
        assert_eq!(provider.get_int("key3"), Some(42));
        assert!(provider.has_key("key1"));
        assert!(!provider.has_key("nonexistent"));
    }
}
