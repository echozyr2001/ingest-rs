//! # ingest-execution
//!
//! Core execution engine for the Inngest durable functions platform.
//!
//! This crate provides the execution runtime that orchestrates function execution,
//! manages step coordination, implements retry logic, and controls concurrency.
//!
//! ## Key Components
//!
//! - **ExecutionEngine**: Main execution orchestrator
//! - **StepExecutor**: Individual step execution
//! - **RetryManager**: Intelligent retry logic
//! - **ConcurrencyController**: Resource and flow control
//! - **ExecutionContext**: Execution environment and state
//!
//! ## Usage
//!
//! ```rust
//! use ingest_execution::ExecutionRequest;
//! use ingest_core::{Function, Event};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create function and event
//! let function = Function::new("test-function");
//! let event = Event::new("test.event", serde_json::json!({"key": "value"}));
//!
//! // Create execution request
//! let request = ExecutionRequest::new(function, event);
//! # Ok(())
//! # }
//! ```

pub mod concurrency;
pub mod context;
pub mod engine;
pub mod error;
pub mod executor;
pub mod retry;
pub mod types;

// Re-export public API
pub use context::{ExecutionContext, StepContext};
pub use engine::ExecutionEngine;
pub use error::{ExecutionError, Result};
pub use executor::{StepExecutor, StepExecutorRegistry};
pub use retry::RetryManager;
pub use types::{ExecutionRequest, ExecutionResult, ExecutionStatus};

// Re-export commonly used types from dependencies
pub use ingest_core::{Event, Function, Step, StepOutput, StepStatus};
