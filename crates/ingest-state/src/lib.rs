//! # ingest-state
//!
//! State management system for the Inngest durable functions platform.
//!
//! This crate provides comprehensive state management capabilities including:
//! - Persistent state storage with PostgreSQL backend
//! - State transitions with validation
//! - Snapshot system for point-in-time recovery
//! - Optimistic locking for concurrency control
//! - Recovery mechanisms and rollback functionality
//! - Performance optimization with Redis caching
//!
//! ## Usage
//!
//! ```rust
//! use ingest_state::{StateManager, ExecutionState, ExecutionStatus};
//! use ingest_core::{generate_id_with_prefix, FunctionId};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a state manager
//! let config = ingest_config::Config::load()?;
//! let state_manager = StateManager::new(config).await?;
//!
//! // Create a new execution state
//! let run_id = generate_id_with_prefix("run");
//! let function_id = generate_id_with_prefix("fn");
//! let state = ExecutionState::new(run_id.clone(), function_id);
//!
//! // Save the state
//! state_manager.save_state(&state).await?;
//!
//! // Update the state
//! if let Some(mut updated_state) = state_manager.get_state(&run_id).await? {
//!     updated_state.set_status(ExecutionStatus::Running);
//!     state_manager.update_state(&updated_state).await?;
//! }
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod error;
pub mod locking;
pub mod manager;
pub mod recovery;
pub mod snapshots;
pub mod storage;
pub mod transitions;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export public API
pub use cache::StateCache;
pub use error::{Result, StateError};
pub use locking::OptimisticLockManager;
pub use manager::StateManager;
pub use recovery::RecoveryManager;
pub use snapshots::SnapshotManager;
pub use storage::{CacheStateStorage, PostgresStateStorage, StateStorage};
pub use transitions::TransitionManager;
pub use types::{
    ExecutionContext, ExecutionState, ExecutionStatus, StateSnapshot, StateTransition,
};

// Type aliases for convenience
pub use ingest_core::{DateTime, Id, Json};
