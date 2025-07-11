//! # ingest-pubsub
//!
//! Publish-subscribe messaging system for the Inngest durable functions platform.
//!
//! This crate provides a comprehensive pub-sub system with topic-based routing,
//! message durability guarantees, subscriber management, and event broadcasting
//! capabilities.
//!
//! ## Key Components
//!
//! - **PubSubManager**: Main coordinator for pub-sub operations
//! - **TopicManager**: Topic lifecycle and routing management
//! - **SubscriptionManager**: Subscriber registration and management
//! - **MessageBroker**: Message routing and delivery
//! - **Publisher/Subscriber**: Core traits for message publishing and consumption
//!
//! ## Features
//!
//! - Topic-based message routing with wildcard support
//! - At-least-once delivery guarantees
//! - Message persistence and durability
//! - Dead letter queue handling
//! - Real-time event broadcasting
//! - Integration with execution components
//!
//! ## Usage
//!
//! ```rust
//! use ingest_pubsub::{Message, Topic};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create topic
//! let topic = Topic::new("function.execution.completed");
//!
//! // Create message
//! let message = Message::new(json!({"function_id": "test", "status": "completed"}));
//! # Ok(())
//! # }
//! ```

pub mod broker;
pub mod error;
pub mod integration;
pub mod manager;
pub mod message;
pub mod subscriber;
pub mod topic;
pub mod traits;
pub mod types;

// Re-export public API
pub use broker::{BrokerStats, InMemoryBroker, MessageBroker, PersistentBroker};
pub use error::{PubSubError, Result};
pub use integration::{ExecutionIntegration, IntegrationStats};
pub use manager::{HealthCheckResult, ManagerStats, PubSubConfig, PubSubManager};
pub use message::Message;
pub use subscriber::{
    MessageReceiver, MessageSender, Subscriber, SubscriberHandle, SubscriptionManager,
};
pub use topic::{Topic, TopicManager, TopicPattern};
pub use traits::{Publisher, SubscriberTrait};
pub use types::{
    DeliveryGuarantee, MessageId, MessageMetadata, SubscriberId, SubscriptionConfig, TopicConfig,
};

// Re-export commonly used types from dependencies
pub use ingest_core::{DateTime, Json, Result as CoreResult};
