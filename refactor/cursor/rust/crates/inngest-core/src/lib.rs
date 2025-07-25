//! Core data structures and traits for Inngest
//!
//! This crate contains the fundamental types and interfaces used throughout
//! the Inngest system. All other crates depend on this one.

pub mod error;
pub mod event;
pub mod function;
pub mod identifier;
pub mod metadata;
pub mod queue;
pub mod state;

// Re-export core types for convenience
pub use error::{Error, Result};
pub use event::Event;
pub use function::Function;
pub use identifier::Identifier;
pub use metadata::Metadata;

/// Common traits used throughout the system
pub mod traits {
    use async_trait::async_trait;

    /// Base trait for all services that can be started and stopped
    #[async_trait]
    pub trait Service: Send + Sync {
        async fn start(&self) -> crate::Result<()>;
        async fn stop(&self) -> crate::Result<()>;
        fn name(&self) -> &str;
    }

    /// Trait for components that can be cloned cheaply (typically Arc-wrapped)
    pub trait CloneService: Service + Clone {}
}

/// Common constants used throughout the system
pub mod constants {
    pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
    pub const MAX_EVENT_SIZE: usize = 512 * 1024; // 512KB
    pub const MAX_STEP_OUTPUT_SIZE: usize = 10 * 1024 * 1024; // 10MB

    // Event names
    pub const EVENT_RECEIVED_NAME: &str = "inngest/events.received";
    pub const FN_FAILED_NAME: &str = "inngest/function.failed";
    pub const FN_FINISHED_NAME: &str = "inngest/function.finished";
    pub const FN_CANCELLED_NAME: &str = "inngest/function.cancelled";
    pub const FN_INVOKE_NAME: &str = "inngest/function.invoke";
    pub const FN_CRON_NAME: &str = "inngest/scheduled.timer";

    // Internal prefixes
    pub const INTERNAL_NAME_PREFIX: &str = "inngest/";
    pub const INNGEST_EVENT_DATA_PREFIX: &str = "inngest";
    pub const INVOKE_CORRELATION_ID: &str = "correlation_id";
}

/// Version information
pub mod version {
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const GIT_HASH: &str = "unknown";

    pub fn print() -> String {
        format!("{VERSION}-dev")
    }
}
