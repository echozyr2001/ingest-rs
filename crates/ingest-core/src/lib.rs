//! # Inngest Core
//!
//! This crate provides the foundational types, traits, and utilities for the Inngest
//! durable functions platform. It defines the core abstractions that other crates
//! build upon.
//!
//! ## Key Components
//!
//! - **Events**: Core event types and handling
//! - **Functions**: Function definitions and metadata
//! - **Steps**: Step execution and coordination
//! - **Identifiers**: Unique ID generation and management
//! - **Traits**: Shared interfaces for extensibility
//! - **Errors**: Common error types and handling

pub mod error;
pub mod event;
pub mod function;
pub mod id;
pub mod step;
pub mod traits;

// Re-export commonly used types
pub use error::{Error, Result};
pub use event::{Event, EventData, EventId};
pub use function::{Function, FunctionConfig, FunctionId, FunctionTrigger};
pub use id::{Id, generate_id, generate_id_with_prefix};
pub use step::{Step, StepConfig, StepId, StepOutput, StepStatus};
pub use traits::{EventHandler, QueueProvider, StateManager};

/// Common type aliases for convenience
pub type DateTime = chrono::DateTime<chrono::Utc>;
pub type Duration = std::time::Duration;
pub type Json = serde_json::Value;
