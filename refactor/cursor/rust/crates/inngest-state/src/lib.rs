//! # Inngest State Management
//!
//! Redis-backed state management system for Inngest workflows, designed to be 100% compatible
//! with the Go implementation.
//!
//! ## Features
//!
//! - **Redis Backend**: High-performance Redis state storage with Lua script atomicity
//! - **Full Compatibility**: JSON serialization and Redis key formats match Go implementation
//! - **Async Support**: Built on tokio for high-performance concurrent operations
//! - **Type Safety**: Strong typing with compile-time guarantees
//! - **Observability**: Comprehensive logging and tracing support
//!
//! ## Usage
//!
//! ```rust,no_run
//! use inngest_state::{RedisStateManager, StateConfig};
//! use inngest_core::{Identifier, Metadata, Event};
//! use chrono::Utc;
//! use uuid::Uuid;
//! use ulid::Ulid;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create state manager
//! let config = StateConfig::default()
//!     .with_redis_url("redis://localhost:6379")?
//!     .with_key_prefix("inngest");
//!
//! let state_manager = RedisStateManager::new(config).await?;
//!
//! // Note: This is just a demonstration of the API
//! // In practice, you would get these values from your application
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod key_generator;
pub mod lua_scripts;
pub mod redis_client;
pub mod serialization;
pub mod state_manager;

// Re-export core types
pub use config::StateConfig;
pub use error::{StateError, StateResult};
pub use state_manager::RedisStateManager;

// Re-export from inngest-core for convenience
pub use inngest_core::{
    Identifier, Metadata, Result,
    state::{CreateStateInput, MemoizedStep, State, StateLoader, StateManager, StateMutator},
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify that all key exports are available
        let _config: Option<StateConfig> = None;
        let _manager: Option<RedisStateManager> = None;
    }
}
