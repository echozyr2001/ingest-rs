//! # ingest-events
//!
//! Event processing system for the Inngest durable functions platform.
//!
//! This crate provides comprehensive event processing capabilities including:
//! - Event validation and schema checking
//! - Multiple serialization formats (JSON, MessagePack)
//! - Flexible routing with pattern matching and rules
//! - Event filtering and transformation
//! - Stream processing with batching and flow control
//! - Performance monitoring and metrics
//!
//! ## Quick Start
//!
//! ```rust
//! use ingest_events::{
//!     processor::create_basic_processor,
//!     validation::create_basic_validator,
//!     routing::create_basic_routing_engine,
//! };
//! use ingest_core::Event;
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create an event
//!     let event = Event::new("user.login", json!({"user_id": "123"}));
//!     
//!     // Create a processor with validation and routing
//!     let processor = create_basic_processor()
//!         .with_validator(create_basic_validator())
//!         .with_routing_engine(create_basic_routing_engine());
//!     
//!     // Process the event
//!     let result = processor.process_event(event).await?;
//!     println!("Event processed successfully: {}", result.success);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The event processing system follows a pipeline architecture:
//!
//! ```text
//! Event → Validation → Transformation → Routing → Serialization → Handlers
//! ```
//!
//! ### Components
//!
//! - **Validation**: Schema validation, field checking, and custom rules
//! - **Serialization**: JSON, MessagePack, and custom format support
//! - **Routing**: Pattern-based routing with filters and rules
//! - **Processing**: Core event processing pipeline with stages
//! - **Streaming**: Batched processing with flow control and backpressure
//!
//! ## Features
//!
//! ### Event Validation
//!
//! ```rust
//! use ingest_events::validation::{ValidationRules, EventValidator};
//! use ingest_events::routing::patterns::EventPattern;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let rules = ValidationRules::new()
//!     .require_field("user_id")
//!     .with_pattern("name", r"^user\.")?;
//!
//! let validator = EventValidator::new()
//!     .add_rules("user.*", rules);
//! # Ok(())
//! # }
//! ```
//!
//! ### Event Routing
//!
//! ```rust
//! use ingest_events::routing::{
//!     RoutingEngine, RoutingRule, RuleCondition, RouteDestination
//! };
//! use ingest_events::routing::patterns::EventPattern;
//!
//! let destination = RouteDestination::new("user-queue", "queue")
//!     .with_config("queue_name", "user_events");
//!
//! let rule = RoutingRule::new(
//!     "user-events",
//!     "User Events",
//!     RuleCondition::EventName(EventPattern::glob("user.*"))
//! ).with_destination(destination);
//!
//! let engine = RoutingEngine::new().add_rule(rule);
//! ```
//!
//! ### Event Streaming
//!
//! ```rust
//! use ingest_events::stream::EventStreamBuilder;
//! use ingest_events::create_basic_processor;
//! use std::sync::Arc;
//!
//! let processor = Arc::new(create_basic_processor());
//! let stream = EventStreamBuilder::new()
//!     .buffer_size(10000)
//!     .batch_size(100)
//!     .batch_timeout(1000)
//!     .rate_limit(1000)
//!     .build(processor);
//! ```

pub mod error;
pub mod processor;
pub mod routing;
pub mod serialization;
pub mod stream;
pub mod validation;

// Re-export commonly used types
pub use error::{EventError, Result};
pub use processor::{
    EventProcessor, ProcessingResult, ProcessingStage, ProcessingStats, ProcessorConfig,
    create_basic_processor, create_high_performance_processor,
};
pub use routing::{
    CompositeFilter, EventFilter, EventPattern, FieldFilter, FilterManager, FilterOperation,
    PatternMatcher, PatternType, RouteDestination, RoutingEngine, RoutingResult, RoutingRule,
    RuleCondition, create_basic_routing_engine, create_common_filters, create_common_patterns,
};
pub use serialization::{EventSerializer, SerializationFormat, SerializedEvent};
pub use stream::{EventStream, EventStreamBuilder, StreamConfig, StreamStats};
pub use validation::{
    EventValidator, ValidationRules, create_basic_validator, create_strict_validator,
};

/// Event processing prelude for common imports
pub mod prelude {
    pub use crate::{
        EventError, EventFilter, EventPattern, EventProcessor, EventSerializer, EventStream,
        EventValidator, ProcessorConfig, Result, RouteDestination, RoutingEngine, RoutingRule,
        RuleCondition, SerializationFormat, ValidationRules, create_basic_processor,
        create_basic_routing_engine, create_basic_validator,
    };
    pub use ingest_core::{Event, EventData, EventHandler};
}
